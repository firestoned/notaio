// Copyright (c) 2026 Erick Bourgeois, notaio
// SPDX-License-Identifier: Apache-2.0
use std::sync::atomic::{AtomicBool, Ordering};

use http_body_util::BodyExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;

struct Fake {
    ready: AtomicBool,
}

impl Observed for Fake {
    fn ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
    fn metrics(&self) -> String {
        "notaio_fail_open_total{check=\"no-ceiling\"} 0\n# EOF\n".to_string()
    }
}

fn fake(ready: bool) -> Fake {
    Fake {
        ready: AtomicBool::new(ready),
    }
}

#[test]
fn only_three_exact_paths_exist() {
    assert_eq!(route(&Method::GET, "/healthz"), Route::Health);
    assert_eq!(route(&Method::GET, "/readyz"), Route::Ready);
    assert_eq!(route(&Method::GET, "/metrics"), Route::Metrics);
    assert_eq!(route(&Method::HEAD, "/metrics"), Route::Metrics);
    for path in [
        "/",
        "/metrics/",
        "/healthz/x",
        "/debug/pprof",
        "//metrics",
        "/METRICS",
    ] {
        assert_eq!(route(&Method::GET, path), Route::NotFound, "{path}");
    }
}

#[test]
fn nothing_but_reads_is_accepted() {
    for m in [
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
        Method::OPTIONS,
    ] {
        assert_eq!(route(&m, "/metrics"), Route::MethodNotAllowed, "{m}");
    }
}

async fn body(r: Response<Full<Bytes>>) -> String {
    let b = r.into_body().collect().await.unwrap().to_bytes();
    String::from_utf8(b.to_vec()).unwrap()
}

#[tokio::test]
async fn readiness_follows_the_controller() {
    let state = fake(false);
    let r = respond(Route::Ready, &state);
    assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    state.ready.store(true, Ordering::SeqCst);
    assert_eq!(respond(Route::Ready, &state).status(), StatusCode::OK);
    // Liveness does not wait for the watches: a slow first list must not restart the pod.
    assert_eq!(
        respond(Route::Health, &fake(false)).status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn metrics_are_served_as_openmetrics() {
    let r = respond(Route::Metrics, &fake(true));
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(
        r.headers()[header::CONTENT_TYPE],
        crate::metrics::CONTENT_TYPE
    );
    assert!(body(r).await.contains("notaio_fail_open_total"));
}

/// One raw HTTP/1.1 exchange with a real listener.
async fn exchange(addr: std::net::SocketAddr, request: &str) -> String {
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(request.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    out
}

#[tokio::test]
async fn the_listener_serves_and_refuses_over_a_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(serve(listener, Arc::new(fake(true))));

    let ok = exchange(
        addr,
        "GET /metrics HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n",
    )
    .await;
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("notaio_fail_open_total"), "{ok}");

    // A body is not read, and does not change the answer.
    let post = exchange(
        addr,
        "POST /metrics HTTP/1.1\r\nhost: x\r\ncontent-length: 4\r\nconnection: close\r\n\r\nabcd",
    )
    .await;
    assert!(post.starts_with("HTTP/1.1 405"), "{post}");

    let missing = exchange(
        addr,
        "GET /x HTTP/1.1\r\nhost: x\r\nconnection: close\r\n\r\n",
    )
    .await;
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
    server.abort();
}
