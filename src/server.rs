use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::analysis::ANALYSIS_SCHEMA_VERSION;
use crate::analyzer::Analyzer;

#[derive(Clone)]
pub struct AppState {
    analyzer: Arc<Analyzer>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    schema_version: u32,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        schema_version: ANALYSIS_SCHEMA_VERSION,
    })
}

pub fn router(analyzer: Arc<Analyzer>) -> Router {
    let state = AppState { analyzer };
    Router::new()
        .route("/api/health", get(health))
        .with_state(state)
}
