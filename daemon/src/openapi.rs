use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use utoipa::openapi::{server::ServerBuilder, OpenApi as UtoipaOpenApi};
use utoipa::OpenApi;

const OPENCODE_OPENAPI_PATH: &str = "/doc";

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct NightshiftProjectPathResponse {
    path: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
struct NightshiftErrorResponse {
    error: String,
}

#[utoipa::path(
    get,
    path = "/project/absolute_path",
    operation_id = "daemon.project.absolutePath",
    responses((status = 200, description = "Project path", body = NightshiftProjectPathResponse))
)]
#[allow(dead_code)]
fn project_absolute_path_route() {}

#[utoipa::path(
    get,
    path = "/teams",
    operation_id = "daemon.teams.list",
    responses((status = 200, description = "Team summaries", body = [crate::teams::TeamSummary]))
)]
#[allow(dead_code)]
fn teams_route() {}

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
#[allow(dead_code)]
fn member_diff_route() {}

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
#[allow(dead_code)]
fn member_tools_route() {}

#[derive(OpenApi)]
#[openapi(
    paths(
        project_absolute_path_route,
        teams_route,
        member_diff_route,
        member_tools_route
    ),
    components(schemas(
        NightshiftProjectPathResponse,
        NightshiftErrorResponse,
        crate::teams::TeamSummary,
        crate::teams::MemberSummary,
        crate::teams::DiffSummary,
        crate::teams::FileStat,
        crate::teams::TaskSummary,
        crate::teams::ConflictInfo,
        crate::teams::MemberDiffDetail,
        crate::toolcalls::MemberToolHistory,
        crate::toolcalls::ToolCall,
        crate::toolcalls::ToolStats
    ))
)]
struct DaemonApi;

fn daemon_openapi() -> UtoipaOpenApi {
    DaemonApi::openapi()
}

fn merge_openapi(
    mut upstream: UtoipaOpenApi,
    daemon: UtoipaOpenApi,
    proxy_port: u16,
) -> UtoipaOpenApi {
    upstream.merge(daemon);
    upstream.servers = Some(vec![ServerBuilder::new()
        .url(format!("http://localhost:{proxy_port}"))
        .description(Some("nightshift daemon proxy"))
        .build()]);
    upstream
}

pub async fn merged_openapi_spec(opencode_port: u16, proxy_port: u16) -> Result<serde_json::Value> {
    let upstream_url = format!("http://127.0.0.1:{opencode_port}{OPENCODE_OPENAPI_PATH}");
    let upstream: UtoipaOpenApi = reqwest::get(&upstream_url)
        .await
        .with_context(|| format!("failed to fetch upstream openapi from {upstream_url}"))?
        .error_for_status()
        .with_context(|| format!("upstream openapi returned non-success from {upstream_url}"))?
        .json()
        .await
        .context("failed to parse upstream openapi json")?;

    let daemon = daemon_openapi();
    let merged = merge_openapi(upstream, daemon, proxy_port);
    serde_json::to_value(merged).context("failed to serialize merged openapi")
}
