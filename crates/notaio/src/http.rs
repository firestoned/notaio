//! The health and metrics listener (ADR-0010, decisions 1 to 3).
//!
//! Narrow by construction: three fixed paths, `GET` only, the request body never read, no
//! parameters, nothing written. What it serves comes from `Observed`, which reads the controller's
//! own state. Connections are bounded in number and in time.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http::{header, Method, Request, Response, StatusCode};
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

/// The default listen address. `NOTAIO_HTTP_ADDR` overrides it.
pub const DEFAULT_ADDR: &str = "0.0.0.0:8080";

/// Concurrent connections served; one more is closed at once.
const MAX_CONNECTIONS: usize = 16;
/// Time allowed for a request's headers to arrive.
const HEADER_TIMEOUT: Duration = Duration::from_secs(5);
/// Time allowed for a whole connection, however many requests it carries.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// What the listener reports. Implemented by the controller over its caches.
pub trait Observed: Send + Sync + 'static {
    /// True once the controller's watches have their first full list.
    fn ready(&self) -> bool;
    /// The Prometheus text exposition.
    fn metrics(&self) -> String;
}

/// Where a request goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Health,
    Ready,
    Metrics,
    NotFound,
    MethodNotAllowed,
}

/// The route for `method` and `path`. Exact paths only.
pub fn route(method: &Method, path: &str) -> Route {
    let known = match path {
        "/healthz" => Route::Health,
        "/readyz" => Route::Ready,
        "/metrics" => Route::Metrics,
        _ => return Route::NotFound,
    };
    if method == Method::GET || method == Method::HEAD {
        known
    } else {
        Route::MethodNotAllowed
    }
}

fn text(status: StatusCode, content_type: &str, body: String) -> Response<Full<Bytes>> {
    let mut r = Response::new(Full::new(Bytes::from(body)));
    *r.status_mut() = status;
    if let Ok(v) = header::HeaderValue::from_str(content_type) {
        r.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    r
}

/// The response for `route`.
pub fn respond(route: Route, state: &dyn Observed) -> Response<Full<Bytes>> {
    const PLAIN: &str = "text/plain; charset=utf-8";
    match route {
        Route::Health => text(StatusCode::OK, PLAIN, "ok\n".to_string()),
        Route::Ready if state.ready() => text(StatusCode::OK, PLAIN, "ready\n".to_string()),
        Route::Ready => text(
            StatusCode::SERVICE_UNAVAILABLE,
            PLAIN,
            "waiting for the first list of every watched kind\n".to_string(),
        ),
        Route::Metrics => text(
            StatusCode::OK,
            crate::metrics::CONTENT_TYPE,
            state.metrics(),
        ),
        Route::NotFound => text(StatusCode::NOT_FOUND, PLAIN, "not found\n".to_string()),
        Route::MethodNotAllowed => {
            let mut r = text(
                StatusCode::METHOD_NOT_ALLOWED,
                PLAIN,
                "method not allowed\n".to_string(),
            );
            r.headers_mut()
                .insert(header::ALLOW, header::HeaderValue::from_static("GET, HEAD"));
            r
        }
    }
}

/// Serve `listener` until the task is dropped.
pub async fn serve(listener: TcpListener, state: Arc<dyn Observed>) {
    let permits = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "health and metrics listener: accept failed");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let Ok(permit) = permits.clone().try_acquire_owned() else {
            tracing::debug!(%peer, "health and metrics listener: at capacity, closing");
            continue;
        };
        let state = state.clone();
        tokio::spawn(async move {
            let service = service_fn(move |req: Request<Incoming>| {
                // The body is never read: no route takes one.
                let r = respond(route(req.method(), req.uri().path()), state.as_ref());
                async move { Ok::<_, Infallible>(r) }
            });
            let conn = http1::Builder::new()
                .timer(TokioTimer::new())
                .header_read_timeout(HEADER_TIMEOUT)
                .serve_connection(TokioIo::new(stream), service);
            if let Ok(Err(e)) = tokio::time::timeout(CONNECTION_TIMEOUT, conn).await {
                tracing::debug!(%peer, error = %e, "health and metrics connection ended");
            }
            drop(permit);
        });
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
