use anyhow::{anyhow, Context, Result};
use oas3::spec::{
    Components, ObjectOrReference, Operation, Parameter, ParameterIn, PathItem, Server, Spec,
};

const OPENCODE_OPENAPI_PATH: &str = "/doc";

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

pub async fn merged_openapi_spec(
    opencode_port: u16,
    proxy_port: u16,
    daemon_openapi_json: &str,
) -> Result<String> {
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

    let daemon_spec: Spec = oas3::from_json(daemon_openapi_json).map_err(|e| {
        let prefix: String = daemon_openapi_json.chars().take(220).collect();
        anyhow!("failed to parse daemon openapi json: {e}; body_prefix={prefix:?}")
    })?;

    normalize_upstream_spec(&mut upstream_spec);
    let merged = merge_openapi(upstream_spec, daemon_spec, proxy_port);
    serde_json::to_string(&merged).context("failed to serialize merged openapi")
}
