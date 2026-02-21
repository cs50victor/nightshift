use anyhow::{anyhow, Context, Result};
use oas3::spec::{
    Components, ObjectOrReference, Operation, Parameter, ParameterIn, PathItem, Server, Spec,
};
use serde::{Deserialize, Serialize};
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

fn daemon_openapi() -> Result<Spec> {
    let daemon_json = DaemonApi::openapi()
        .to_json()
        .context("failed to serialize daemon openapi")?;
    oas3::from_json(&daemon_json).context("failed to parse daemon openapi as oas3")
}

fn merge_components(into: &mut Components, from: Components) {
    into.schemas.extend(from.schemas);
    into.responses.extend(from.responses);
    into.parameters.extend(from.parameters);
    into.examples.extend(from.examples);
    into.request_bodies.extend(from.request_bodies);
    into.headers.extend(from.headers);
    into.path_items.extend(from.path_items);
    into.security_schemes.extend(from.security_schemes);
    into.links.extend(from.links);
    into.callbacks.extend(from.callbacks);
    into.extensions.extend(from.extensions);
}

fn merge_openapi(mut upstream: Spec, daemon: Spec, proxy_port: u16) -> Spec {
    if let Some(mut daemon_paths) = daemon.paths {
        let upstream_paths = upstream.paths.get_or_insert_default();
        upstream_paths.append(&mut daemon_paths);
    }

    if let Some(daemon_components) = daemon.components {
        if let Some(upstream_components) = upstream.components.as_mut() {
            merge_components(upstream_components, daemon_components);
        } else {
            upstream.components = Some(daemon_components);
        }
    }

    upstream.servers = vec![Server {
        url: format!("http://localhost:{proxy_port}"),
        description: Some("nightshift daemon proxy".to_string()),
        variables: Default::default(),
        extensions: Default::default(),
    }];

    upstream
}

fn normalize_parameter_required(param: &mut Parameter) {
    if param.required.is_none() {
        param.required = Some(matches!(param.location, ParameterIn::Path));
    }
}

fn normalize_parameter_list(params: &mut [ObjectOrReference<Parameter>]) {
    for param in params {
        if let ObjectOrReference::Object(p) = param {
            normalize_parameter_required(p);
        }
    }
}

fn normalize_operation_parameters(op: &mut Operation) {
    normalize_parameter_list(&mut op.parameters);
}

fn normalize_path_item_parameters(path: &mut PathItem) {
    normalize_parameter_list(&mut path.parameters);
    if let Some(op) = path.get.as_mut() {
        normalize_operation_parameters(op);
    }
    if let Some(op) = path.put.as_mut() {
        normalize_operation_parameters(op);
    }
    if let Some(op) = path.post.as_mut() {
        normalize_operation_parameters(op);
    }
    if let Some(op) = path.delete.as_mut() {
        normalize_operation_parameters(op);
    }
    if let Some(op) = path.options.as_mut() {
        normalize_operation_parameters(op);
    }
    if let Some(op) = path.head.as_mut() {
        normalize_operation_parameters(op);
    }
    if let Some(op) = path.patch.as_mut() {
        normalize_operation_parameters(op);
    }
    if let Some(op) = path.trace.as_mut() {
        normalize_operation_parameters(op);
    }
}

fn normalize_upstream_spec(spec: &mut Spec) {
    if let Some(paths) = spec.paths.as_mut() {
        for path in paths.values_mut() {
            normalize_path_item_parameters(path);
        }
    }
}

pub async fn merged_openapi_spec(opencode_port: u16, proxy_port: u16) -> Result<String> {
    let upstream_url = format!("http://127.0.0.1:{opencode_port}{OPENCODE_OPENAPI_PATH}");
    let upstream_raw = reqwest::get(&upstream_url)
        .await
        .with_context(|| format!("failed to fetch upstream openapi from {upstream_url}"))?
        .error_for_status()
        .with_context(|| format!("upstream openapi returned non-success from {upstream_url}"))?
        .text()
        .await
        .context("failed to read upstream openapi body")?;

    let mut upstream_spec: Spec = oas3::from_json(&upstream_raw).map_err(|e| {
        let prefix: String = upstream_raw.chars().take(220).collect();
        anyhow!("failed to parse upstream openapi json: {e}; body_prefix={prefix:?}")
    })?;

    normalize_upstream_spec(&mut upstream_spec);
    let daemon = daemon_openapi()?;
    let merged = merge_openapi(upstream_spec, daemon, proxy_port);
    serde_json::to_string(&merged).context("failed to serialize merged openapi")
}
