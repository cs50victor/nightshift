use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{Either, Full};
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use tokio::net::{TcpListener, TcpStream};

type ProxyBody = Either<Incoming, Full<Bytes>>;

async fn handle(
    mut req: Request<Incoming>,
    opencode_port: u16,
    project_path: &str,
) -> Result<Response<ProxyBody>, Infallible> {
    if req.uri().path() == "/project/absolute_path" {
        let body = format!(r#"{{"path":"{}"}}"#, project_path);
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Either::Right(Full::new(Bytes::from(body))))
            .unwrap());
    }

    let is_upgrade = req.headers().contains_key(hyper::header::UPGRADE);
    let on_client_upgrade = if is_upgrade {
        Some(hyper::upgrade::on(&mut req))
    } else {
        None
    };

    match forward(req, opencode_port).await {
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

async fn forward(mut req: Request<Incoming>, opencode_port: u16) -> Result<Response<Incoming>> {
    let path = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    *req.uri_mut() = path.parse()?;

    let stream = TcpStream::connect(("127.0.0.1", opencode_port))
        .await
        .context("opencode unreachable")?;
    let io = TokioIo::new(stream);
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::spawn(conn.with_upgrades());

    let resp = sender.send_request(req).await?;
    Ok(resp)
}

pub async fn serve(opencode_port: u16, listen_port: u16, project_path: String) -> Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", listen_port))
        .await
        .with_context(|| format!("failed to bind proxy to :{listen_port}"))?;
    tracing::info!("proxy listening on :{listen_port} -> :{opencode_port}");

    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let path = project_path.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req: Request<Incoming>| {
                let port = opencode_port;
                let p = path.clone();
                async move { handle(req, port, &p).await }
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
