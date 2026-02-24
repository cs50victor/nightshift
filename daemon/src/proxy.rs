use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

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
    db: Arc<SqlitePool>,
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

#[derive(serde::Serialize, serde::Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct NightshiftGitDiffResponse {
    diff: String,
    worktree: String,
}

#[derive(Debug, Deserialize, IntoParams)]
struct GitDiffQuery {
    path: Option<String>,
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
    responses((status = 200, description = "Team summaries", body = [crate::teams_repo::TeamSummary]))
)]
async fn get_teams(State(state): State<AppState>) -> Response {
    match crate::teams_repo::list_teams(&state.db).await {
        Ok(teams) => Json(teams).into_response(),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
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
        (status = 200, description = "Member diff", body = crate::diff::MemberDiffDetail),
        (status = 404, description = "Not found", body = NightshiftErrorResponse)
    )
)]
async fn get_member_diff(
    State(state): State<AppState>,
    Path((team, name)): Path<(String, String)>,
) -> Response {
    match crate::teams_repo::get_member_diff_cwd(&state.db, &team, &name).await {
        Ok(Some((cwd, baseline))) => {
            let diff = if let Some(ref b) = baseline {
                crate::diff::compute_diff_full(&cwd, b)
                    .await
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let detail = crate::diff::MemberDiffDetail {
                name,
                team,
                cwd: cwd.clone(),
                baseline_commit: baseline,
                current_commit: crate::diff::git_head(&cwd).await,
                diff,
            };
            Json(detail).into_response()
        }
        Ok(None) => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.into()),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
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
    match crate::teams_repo::get_member_tools(&state.db, &team, &name).await {
        Ok(Some(tools)) => Json(tools).into_response(),
        Ok(None) => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.into()),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn git_rev_parse(cwd: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(cwd)
        .output()
        .await
        .context("failed to run git rev-parse")?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "not a git repository: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

async fn git_diff_tracked(cwd: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(cwd)
        .output()
        .await
        .context("failed to run git diff HEAD")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git diff HEAD failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn git_diff_untracked(cwd: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .current_dir(cwd)
        .output()
        .await
        .context("failed to list untracked files")?;
    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let mut combined = String::new();
    for raw in output
        .stdout
        .split(|b| *b == 0)
        .filter(|part| !part.is_empty())
    {
        let file = String::from_utf8_lossy(raw).to_string();
        let diff_output = Command::new("git")
            .args(["diff", "--no-index", "--", "/dev/null", &file])
            .current_dir(cwd)
            .output()
            .await
            .with_context(|| format!("failed to diff untracked file {file}"))?;

        if diff_output.status.success() || diff_output.status.code() == Some(1) {
            combined.push_str(&String::from_utf8_lossy(&diff_output.stdout));
        } else {
            return Err(anyhow::anyhow!(
                "git diff --no-index failed for {file}: {}",
                String::from_utf8_lossy(&diff_output.stderr).trim()
            ));
        }
    }

    Ok(combined)
}

#[utoipa::path(
    get,
    path = "/git/diff",
    operation_id = "daemon.git.diff",
    params(GitDiffQuery),
    responses(
        (status = 200, description = "Git diff for repository", body = NightshiftGitDiffResponse),
        (status = 400, description = "Missing path query", body = NightshiftErrorResponse),
        (status = 422, description = "Not a git repository", body = NightshiftErrorResponse),
        (status = 500, description = "Failed to compute git diff", body = NightshiftErrorResponse)
    )
)]
async fn get_git_diff(axum::extract::Query(query): axum::extract::Query<GitDiffQuery>) -> Response {
    let Some(cwd) = query.path else {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": "missing required query parameter: path"}).to_string(),
        );
    };

    if let Err(e) = git_rev_parse(&cwd).await {
        return json_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({"error": format!("Not a git repository: {cwd}") , "detail": format!("{e:#}")})
                .to_string(),
        );
    }

    match tokio::try_join!(git_diff_tracked(&cwd), git_diff_untracked(&cwd)) {
        Ok((tracked, untracked)) => Json(NightshiftGitDiffResponse {
            diff: format!("{tracked}{untracked}"),
            worktree: cwd,
        })
        .into_response(),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("failed to get git diff: {e:#}")}).to_string(),
        ),
    }
}

async fn create_team(
    State(state): State<AppState>,
    Json(req): Json<crate::teams_repo::CreateTeamRequest>,
) -> Response {
    match crate::teams_repo::create_team(&state.db, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn delete_team(
    State(state): State<AppState>,
    Json(req): Json<crate::teams_repo::DeleteTeamRequest>,
) -> Response {
    match crate::teams_repo::delete_team(&state.db, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn spawn_teammate(
    State(state): State<AppState>,
    Json(req): Json<crate::teams_repo::SpawnTeammateRequest>,
) -> Response {
    match crate::teams_repo::spawn_teammate(&state.db, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn kill_teammate(
    State(state): State<AppState>,
    Json(req): Json<crate::teams_repo::KillTeammateRequest>,
) -> Response {
    match crate::teams_repo::kill_teammate(&state.db, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<crate::teams_repo::CreateTaskRequest>,
) -> Response {
    match crate::teams_repo::create_task(&state.db, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn update_task(
    State(state): State<AppState>,
    Json(req): Json<crate::teams_repo::UpdateTaskRequest>,
) -> Response {
    match crate::teams_repo::update_task(&state.db, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn send_message(
    State(state): State<AppState>,
    Json(req): Json<crate::teams_repo::SendMessageRequest>,
) -> Response {
    match crate::teams_repo::send_message(&state.db, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn read_inbox(
    State(state): State<AppState>,
    Json(req): Json<crate::teams_repo::ReadInboxRequest>,
) -> Response {
    match crate::teams_repo::mark_read(&state.db, req).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => json_response(
            StatusCode::BAD_REQUEST,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn get_team_snapshot(State(state): State<AppState>, Path(team): Path<String>) -> Response {
    match crate::teams_repo::get_team_snapshot(&state.db, &team).await {
        Ok(Some(snapshot)) => Json(snapshot).into_response(),
        Ok(None) => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.into()),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn get_activity(
    State(state): State<AppState>,
    Path(team): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let since_ms = params.get("since_ms").and_then(|v| v.parse::<u64>().ok());
    let limit = params.get("limit").and_then(|v| v.parse::<u32>().ok());
    match crate::teams_repo::get_activity(&state.db, &team, since_ms, limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
    }
}

async fn get_member_timeline(
    State(state): State<AppState>,
    Path((team, name)): Path<(String, String)>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let since_ms = params.get("since_ms").and_then(|v| v.parse::<u64>().ok());
    let limit = params.get("limit").and_then(|v| v.parse::<u32>().ok());
    match crate::teams_repo::get_member_timeline(&state.db, &team, &name, since_ms, limit).await {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}).to_string(),
        ),
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
        .routes(routes!(get_git_diff))
        .split_for_parts();

    documented_router
        .route("/doc", get(get_openapi_spec))
        .route("/openapi.json", get(get_openapi_spec))
        .route("/teams/{team}/snapshot", get(get_team_snapshot))
        .route("/teams/{team}/activity", get(get_activity))
        .route(
            "/teams/{team}/members/{name}/timeline",
            get(get_member_timeline),
        )
        .route("/internal/teams/create", post(create_team))
        .route("/internal/teams/delete", post(delete_team))
        .route("/internal/teammates/spawn", post(spawn_teammate))
        .route("/internal/teammates/kill", post(kill_teammate))
        .route("/internal/tasks/create", post(create_task))
        .route("/internal/tasks/update", post(update_task))
        .route("/internal/messages/send", post(send_message))
        .route("/internal/inbox/read", post(read_inbox))
        .fallback(any(proxy_fallback))
}

fn daemon_openapi_json() -> Result<String> {
    let (_, daemon_openapi) = OpenApiRouter::new()
        .routes(routes!(get_project_absolute_path))
        .routes(routes!(get_teams))
        .routes(routes!(get_member_diff))
        .routes(routes!(get_member_tools))
        .routes(routes!(get_git_diff))
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
    db: Arc<SqlitePool>,
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
        db,
    });

    axum::serve(listener, app)
        .await
        .with_context(|| format!("axum server exited on :{listen_port}"))
}
