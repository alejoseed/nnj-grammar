use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nnj_grammar::analyzer::{Analyzer, AnalyzerConfig};
use nnj_grammar::server::{ensure_loopback, router};
use tower::ServiceExt;

fn embedded_analyzer() -> Arc<Analyzer> {
    Arc::new(Analyzer::new(AnalyzerConfig::default()).expect("embedded analyzer should initialize"))
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("readable body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("valid JSON body")
}

#[tokio::test]
async fn health_reports_schema_version() {
    let app = router(embedded_analyzer());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["status"], "ok");
    assert_eq!(json["schema_version"], 3);
}

fn analyze_request(body: &'static str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/analyze")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn analyze_returns_schema_v3_document_for_embedded_soshite() {
    let app = router(embedded_analyzer());
    let response = app
        .oneshot(analyze_request(r#"{"text":"そして"}"#))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["schema_version"], 3);
    assert_eq!(json["input"], "そして");
    assert!(json["primary_matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|matched| matched["rule_name"] == "そして、～"));
}

async fn error_json(response: axum::response::Response) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    (status, body_json(response).await)
}

#[tokio::test]
async fn analyze_rejects_malformed_json_body() {
    let app = router(embedded_analyzer());
    let (status, json) = error_json(app.oneshot(analyze_request("not json")).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_json");
}

#[tokio::test]
async fn analyze_rejects_unknown_fields() {
    let app = router(embedded_analyzer());
    let (status, json) = error_json(
        app.oneshot(analyze_request(r#"{"text":"そして","extra":1}"#))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_json");
}

#[tokio::test]
async fn analyze_rejects_missing_text_field() {
    let app = router(embedded_analyzer());
    let (status, json) = error_json(app.oneshot(analyze_request(r#"{}"#)).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_json");
}

#[tokio::test]
async fn analyze_rejects_wrong_field_type() {
    let app = router(embedded_analyzer());
    let (status, json) =
        error_json(app.oneshot(analyze_request(r#"{"text":5}"#)).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_json");
}

#[tokio::test]
async fn analyze_rejects_unsupported_content_type() {
    let app = router(embedded_analyzer());
    let request = Request::builder()
        .method("POST")
        .uri("/api/analyze")
        .header("content-type", "text/plain")
        .body(Body::from(r#"{"text":"そして"}"#))
        .unwrap();
    let (status, json) = error_json(app.oneshot(request).await.unwrap()).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(json["error"]["code"], "unsupported_media_type");
}

#[tokio::test]
async fn analyze_rejects_empty_text() {
    let app = router(embedded_analyzer());
    let (status, json) = error_json(
        app.oneshot(analyze_request(r#"{"text":"   "}"#))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "empty_input");
}

#[tokio::test]
async fn analyze_rejects_oversized_raw_body() {
    let app = router(embedded_analyzer());
    let oversized = format!(r#"{{"text":"{}"}}"#, "a".repeat(600 * 1024));
    let request = Request::builder()
        .method("POST")
        .uri("/api/analyze")
        .header("content-type", "application/json")
        .body(Body::from(oversized))
        .unwrap();
    let (status, json) = error_json(app.oneshot(request).await.unwrap()).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(json["error"]["code"], "request_too_large");
}

#[tokio::test]
async fn unmatched_path_returns_json_404() {
    let app = router(embedded_analyzer());
    let (status, json) = error_json(
        app.oneshot(
            Request::builder()
                .uri("/api/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "not_found");
}

#[tokio::test]
async fn wrong_method_on_known_route_returns_json_405() {
    let app = router(embedded_analyzer());
    let (status, json) = error_json(
        app.oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/analyze")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(json["error"]["code"], "method_not_allowed");
}

use std::time::Duration;

use nnj_grammar::server::serve;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener as TokioTcpListener;

#[tokio::test]
async fn real_loopback_listener_serves_health() {
    let listener = TokioTcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral loopback port");
    let addr = listener.local_addr().expect("listener address");
    let app = router(embedded_analyzer());
    let server_task = tokio::spawn(async move {
        let _ = serve(listener, app).await;
    });

    let mut stream =
        tokio::time::timeout(Duration::from_secs(5), tokio::net::TcpStream::connect(addr))
            .await
            .expect("connect within timeout")
            .expect("connect to loopback server");

    stream
        .write_all(
            format!("GET /api/health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .expect("write request");

    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response))
        .await
        .expect("read within timeout")
        .expect("read response");
    let response = String::from_utf8_lossy(&response);

    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("\"status\":\"ok\""));

    server_task.abort();
}

#[tokio::test]
async fn serve_rejects_a_non_loopback_socket_address() {
    assert!(ensure_loopback("0.0.0.0:7878".parse().unwrap()).is_err());
}
