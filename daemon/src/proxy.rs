use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::teams::TeamsHandle;

const STARTUP_RETRY_WINDOW: Duration = Duration::from_secs(8);
const STARTUP_MAX_RETRIES: u32 = 5;
const STARTUP_RETRY_DELAY: Duration = Duration::from_millis(200);

#[derive(Clone)]
struct AppState {
    opencode_port: u16,
    proxy_port: u16,
    project_path: Arc<str>,
    daemon_openapi_json: Arc<str>,
    start_time: std::time::Instant,
    teams: TeamsHandle,
}

#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct NightshiftProjectPathResponse {
    path: String,
}

#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct NightshiftErrorResponse {
    error: String,
}

fn json_response(status: StatusCode, body: String) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

#[utoipa::path(
    get,
    path = "/project/absolute_path",
    operation_id = "daemon.project.absolutePath",
    responses((status = 200, description = "Project path", body = NightshiftProjectPathResponse))
)]
async fn get_project_absolute_path(State(state): State<AppState>) -> impl IntoResponse {
    Json(NightshiftProjectPathResponse {
        path: state.project_path.to_string(),
    })
}

#[utoipa::path(
    get,
    path = "/teams",
    operation_id = "daemon.teams.list",
    responses((status = 200, description = "Team summaries", body = [crate::teams::TeamSummary]))
)]
async fn get_teams(State(state): State<AppState>) -> impl IntoResponse {
    Json(crate::teams::get_teams_summary(&state.teams).await)
}

#[utoipa::path(
    get,
    path = "/teams/{team}/members/{name}/diff",
    operation_id = "daemon.teams.member.diff",
    params(
        ("team" = String, Path, description = "Team name"),
        ("name" = String, Path, description = "Member name")
    ),
    responses(
        (status = 200, description = "Member diff", body = crate::teams::MemberDiffDetail),
        (status = 404, description = "Not found", body = NightshiftErrorResponse)
    )
)]
async fn get_member_diff(
    State(state): State<AppState>,
    Path((team, name)): Path<(String, String)>,
) -> Response {
    match crate::teams::get_member_diff(&state.teams, &team, &name).await {
        Some(diff) => Json(diff).into_response(),
        None => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.into()),
    }
}

#[utoipa::path(
    get,
    path = "/teams/{team}/members/{name}/tools",
    operation_id = "daemon.teams.member.tools",
    params(
        ("team" = String, Path, description = "Team name"),
        ("name" = String, Path, description = "Member name")
    ),
    responses(
        (status = 200, description = "Member tool history", body = crate::toolcalls::MemberToolHistory),
        (status = 404, description = "Not found", body = NightshiftErrorResponse)
    )
)]
async fn get_member_tools(
    State(state): State<AppState>,
    Path((team, name)): Path<(String, String)>,
) -> Response {
    match crate::teams::get_member_tools(&state.teams, &team, &name).await {
        Some(tools) => Json(tools).into_response(),
        None => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.into()),
    }
}

async fn get_openapi_spec(State(state): State<AppState>) -> Response {
    match crate::openapi::merged_openapi_spec(
        state.opencode_port,
        state.proxy_port,
        &state.daemon_openapi_json,
    )
    .await
    {
        Ok(spec) => json_response(StatusCode::OK, spec),
        Err(e) => {
            tracing::warn!("failed to build merged openapi spec: {e:#}");
            json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "error": "failed to build openapi spec",
                    "detail": format!("{e:#}"),
                })
                .to_string(),
            )
        }
    }
}

async fn proxy_fallback(State(state): State<AppState>, mut req: Request) -> Response {
    let is_upgrade = req.headers().contains_key(hyper::header::UPGRADE);
    let on_client_upgrade = if is_upgrade {
        Some(hyper::upgrade::on(&mut req))
    } else {
        None
    };

    match forward(req, state.opencode_port, state.start_time).await {
        Ok(mut resp) => {
            if resp.status() == StatusCode::SWITCHING_PROTOCOLS {
                if let Some(on_client) = on_client_upgrade {
                    let on_upstream = hyper::upgrade::on(&mut resp);
                    tokio::spawn(async move {
                        match tokio::try_join!(on_client, on_upstream) {
                            Ok((client, upstream)) => {
                                let mut client = TokioIo::new(client);
                                let mut upstream = TokioIo::new(upstream);
                                if let Err(e) =
                                    tokio::io::copy_bidirectional(&mut client, &mut upstream).await
                                {
                                    tracing::debug!("ws pipe closed: {e}");
                                }
                            }
                            Err(e) => tracing::warn!("upgrade failed: {e}"),
                        }
                    });
                }
            }
            resp.map(Body::new)
        }
        Err(e) => {
            tracing::warn!("proxy error: {e:#}");
            (StatusCode::BAD_GATEWAY, format!("proxy error: {e}")).into_response()
        }
    }
}

async fn forward(
    mut req: Request,
    opencode_port: u16,
    start_time: std::time::Instant,
) -> Result<hyper::Response<Incoming>> {
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    *req.uri_mut() = path.parse()?;

    let stream = {
        let mut attempts = 0u32;
        loop {
            match TcpStream::connect(("127.0.0.1", opencode_port)).await {
                Ok(s) => break s,
                Err(e)
                    if e.kind() == std::io::ErrorKind::ConnectionRefused
                        && attempts < STARTUP_MAX_RETRIES
                        && start_time.elapsed() < STARTUP_RETRY_WINDOW =>
                {
                    attempts += 1;
                    tracing::debug!(
                        "opencode not ready yet (attempt {}), retrying in {:?}",
                        attempts,
                        STARTUP_RETRY_DELAY
                    );
                    tokio::time::sleep(STARTUP_RETRY_DELAY).await;
                }
                Err(e) => return Err(e).context("opencode unreachable"),
            }
        }
    };

    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::spawn(async move {
        if let Err(e) = conn.with_upgrades().await {
            tracing::debug!("upstream connection ended: {e}");
        }
    });

    let resp = sender.send_request(req).await?;
    Ok(resp)
}

fn api_router() -> Router<AppState> {
    let (documented_router, _) = OpenApiRouter::new()
        .routes(routes!(get_project_absolute_path))
        .routes(routes!(get_teams))
        .routes(routes!(get_member_diff))
        .routes(routes!(get_member_tools))
        .split_for_parts();

    documented_router
        .route("/doc", get(get_openapi_spec))
        .route("/openapi.json", get(get_openapi_spec))
        .fallback(any(proxy_fallback))
}

fn daemon_openapi_json() -> Result<String> {
    let (_, daemon_openapi) = OpenApiRouter::new()
        .routes(routes!(get_project_absolute_path))
        .routes(routes!(get_teams))
        .routes(routes!(get_member_diff))
        .routes(routes!(get_member_tools))
        .split_for_parts();
    daemon_openapi
        .to_json()
        .context("failed to serialize axum daemon openapi")
}

pub async fn serve(
    opencode_port: u16,
    listen_port: u16,
    project_path: String,
    start_time: std::time::Instant,
    teams: TeamsHandle,
) -> Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", listen_port))
        .await
        .with_context(|| format!("failed to bind proxy to :{listen_port}"))?;

    tracing::info!("proxy listening on :{listen_port} -> :{opencode_port}");

    let daemon_openapi_json = daemon_openapi_json()?;

    let app = api_router().with_state(AppState {
        opencode_port,
        proxy_port: listen_port,
        project_path: Arc::<str>::from(project_path),
        daemon_openapi_json: Arc::<str>::from(daemon_openapi_json),
        start_time,
        teams,
    });

    axum::serve(listener, app)
        .await
        .with_context(|| format!("axum server exited on :{listen_port}"))
}
