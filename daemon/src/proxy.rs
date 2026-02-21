use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{Either, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};

use crate::teams::TeamsHandle;

type ProxyBody = Either<Incoming, Full<Bytes>>;

const STARTUP_RETRY_WINDOW: Duration = Duration::from_secs(8);
const STARTUP_MAX_RETRIES: u32 = 5;
const STARTUP_RETRY_DELAY: Duration = Duration::from_millis(200);
const OPENCODE_OPENAPI_PATH: &str = "/doc";

fn json_response(status: StatusCode, body: String) -> Response<ProxyBody> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Either::Right(Full::new(Bytes::from(body))))
        .unwrap()
}

fn internal_error_response(msg: &str) -> Response<ProxyBody> {
    json_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!(r#"{{"error":"{}"}}"#, msg.replace('"', "\\\"")),
    )
}

fn daemon_openapi_paths() -> serde_json::Value {
    serde_json::json!({
        "/project/absolute_path": {
            "get": {
                "summary": "Get project absolute path",
                "description": "Return the daemon project path used by this proxy instance.",
                "operationId": "daemon.project.absolutePath",
                "responses": {
                    "200": {
                        "description": "Project path",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["path"],
                                    "properties": {
                                        "path": { "type": "string" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        "/teams": {
            "get": {
                "summary": "List teams",
                "description": "List active and archived team summaries discovered from local claude-teams state.",
                "operationId": "daemon.teams.list",
                "responses": {
                    "200": {
                        "description": "Team summaries",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "additionalProperties": true
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        "/teams/{team}/members/{name}/diff": {
            "get": {
                "summary": "Get member diff",
                "description": "Return git diff details for a team member since captured baseline.",
                "operationId": "daemon.teams.member.diff",
                "parameters": [
                    {
                        "name": "team",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    },
                    {
                        "name": "name",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    }
                ],
                "responses": {
                    "200": {
                        "description": "Member diff",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "additionalProperties": true
                                }
                            }
                        }
                    },
                    "404": {
                        "description": "Not found",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["error"],
                                    "properties": {
                                        "error": { "type": "string" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        "/teams/{team}/members/{name}/tools": {
            "get": {
                "summary": "Get member tool history",
                "description": "Return tool call history and aggregate stats for a team member.",
                "operationId": "daemon.teams.member.tools",
                "parameters": [
                    {
                        "name": "team",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    },
                    {
                        "name": "name",
                        "in": "path",
                        "required": true,
                        "schema": { "type": "string" }
                    }
                ],
                "responses": {
                    "200": {
                        "description": "Member tool history",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "additionalProperties": true
                                }
                            }
                        }
                    },
                    "404": {
                        "description": "Not found",
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "required": ["error"],
                                    "properties": {
                                        "error": { "type": "string" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

async fn merged_openapi_spec(opencode_port: u16, proxy_port: u16) -> Result<serde_json::Value> {
    let upstream_url = format!("http://127.0.0.1:{opencode_port}{OPENCODE_OPENAPI_PATH}");
    let mut spec: serde_json::Value = reqwest::get(&upstream_url)
        .await
        .with_context(|| format!("failed to fetch upstream openapi from {upstream_url}"))?
        .error_for_status()
        .with_context(|| format!("upstream openapi returned non-success from {upstream_url}"))?
        .json()
        .await
        .context("failed to parse upstream openapi json")?;

    let paths = spec
        .as_object_mut()
        .and_then(|o| o.get_mut("paths"))
        .and_then(|p| p.as_object_mut())
        .context("upstream openapi missing object 'paths'")?;

    if let Some(daemon_paths) = daemon_openapi_paths().as_object() {
        for (path, item) in daemon_paths {
            paths.insert(path.clone(), item.clone());
        }
    }

    if let Some(obj) = spec.as_object_mut() {
        obj.insert(
            "servers".to_string(),
            serde_json::json!([
                {
                    "url": format!("http://localhost:{proxy_port}"),
                    "description": "nightshift daemon proxy"
                }
            ]),
        );
    }

    Ok(spec)
}

fn parse_team_member_path<'a>(path: &'a str, suffix: &str) -> Option<(&'a str, &'a str)> {
    // /teams/{team}/members/{name}/{suffix}
    let path = path.strip_prefix("/teams/")?;
    let (team, rest) = path.split_once('/')?;
    let rest = rest.strip_prefix("members/")?;
    let (name, rest) = rest.split_once('/')?;
    if rest == suffix {
        Some((team, name))
    } else {
        None
    }
}

async fn handle(
    mut req: Request<Incoming>,
    opencode_port: u16,
    proxy_port: u16,
    project_path: &str,
    start_time: std::time::Instant,
    teams: TeamsHandle,
) -> Result<Response<ProxyBody>, Infallible> {
    if (req.uri().path() == "/openapi.json" || req.uri().path() == "/doc")
        && *req.method() == Method::GET
    {
        return Ok(match merged_openapi_spec(opencode_port, proxy_port).await {
            Ok(spec) => json_response(
                StatusCode::OK,
                serde_json::to_string(&spec).unwrap_or_else(|_| "{}".into()),
            ),
            Err(e) => {
                tracing::warn!("failed to build merged openapi spec: {e}");
                internal_error_response("failed to build openapi spec")
            }
        });
    }

    if req.uri().path() == "/project/absolute_path" {
        let body = format!(r#"{{"path":"{}"}}"#, project_path);
        return Ok(json_response(StatusCode::OK, body));
    }

    if req.uri().path() == "/teams" && *req.method() == Method::GET {
        let summaries = crate::teams::get_teams_summary(&teams).await;
        let body = serde_json::to_string(&summaries).unwrap_or_else(|_| "[]".into());
        return Ok(json_response(StatusCode::OK, body));
    }

    if let Some((team, name)) = parse_team_member_path(req.uri().path(), "diff") {
        if *req.method() == Method::GET {
            return Ok(
                match crate::teams::get_member_diff(&teams, team, name).await {
                    Some(d) => {
                        let body = serde_json::to_string(&d).unwrap_or_else(|_| "{}".into());
                        json_response(StatusCode::OK, body)
                    }
                    None => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.into()),
                },
            );
        }
    }

    if let Some((team, name)) = parse_team_member_path(req.uri().path(), "tools") {
        if *req.method() == Method::GET {
            return Ok(
                match crate::teams::get_member_tools(&teams, team, name).await {
                    Some(t) => {
                        let body = serde_json::to_string(&t).unwrap_or_else(|_| "{}".into());
                        json_response(StatusCode::OK, body)
                    }
                    None => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#.into()),
                },
            );
        }
    }

    let is_upgrade = req.headers().contains_key(hyper::header::UPGRADE);
    let on_client_upgrade = if is_upgrade {
        Some(hyper::upgrade::on(&mut req))
    } else {
        None
    };

    match forward(req, opencode_port, start_time).await {
        Ok(mut resp) => {
            if resp.status() == StatusCode::SWITCHING_PROTOCOLS {
                if let Some(on_client) = on_client_upgrade {
                    let on_upstream = hyper::upgrade::on(&mut resp);
                    tokio::spawn(async move {
                        match tokio::try_join!(on_client, on_upstream) {
                            Ok((client, upstream)) => {
                                let mut c = TokioIo::new(client);
                                let mut u = TokioIo::new(upstream);
                                if let Err(e) = tokio::io::copy_bidirectional(&mut c, &mut u).await
                                {
                                    tracing::debug!("ws pipe closed: {e}");
                                }
                            }
                            Err(e) => tracing::warn!("upgrade failed: {e}"),
                        }
                    });
                }
            }
            Ok(resp.map(Either::Left))
        }
        Err(e) => {
            tracing::warn!("proxy error: {e}");
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Either::Right(Full::new(Bytes::from(format!(
                    "proxy error: {e}"
                )))))
                .unwrap())
        }
    }
}

async fn forward(
    mut req: Request<Incoming>,
    opencode_port: u16,
    start_time: std::time::Instant,
) -> Result<Response<Incoming>> {
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    *req.uri_mut() = path.parse()?;

    // NOTE(victor): Retry ECONNREFUSED during startup window so requests arriving
    // before opencode is fully HTTP-ready don't immediately 502.
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
    tokio::spawn(conn.with_upgrades());

    let resp = sender.send_request(req).await?;
    Ok(resp)
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

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let path = project_path.clone();
        let st = start_time;
        let teams = teams.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req: Request<Incoming>| {
                let port = opencode_port;
                let p = path.clone();
                let st = st;
                let teams = teams.clone();
                async move { handle(req, port, listen_port, &p, st, teams).await }
            });
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .with_upgrades()
                .await
            {
                tracing::warn!("connection error: {e}");
            }
        });
    }
}
