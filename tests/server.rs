use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nnj_grammar::analyzer::{Analyzer, AnalyzerConfig};
use nnj_grammar::server::router;
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
    assert_eq!(json["schema_version"], 1);
}
