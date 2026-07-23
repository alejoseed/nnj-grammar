use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::analysis::ANALYSIS_SCHEMA_VERSION;
use crate::analyzer::Analyzer;

#[derive(Clone)]
pub struct AppState {
    analyzer: Arc<Analyzer>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
struct ApiErrorBody<'a> {
    error: ApiErrorDetail<'a>,
}

#[derive(Serialize)]
struct ApiErrorDetail<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ApiErrorBody {
            error: ApiErrorDetail {
                code: self.code,
                message: &self.message,
            },
        };
        (self.status, Json(body)).into_response()
    }
}

fn classify_json_rejection(rejection: JsonRejection) -> ApiError {
    match rejection {
        JsonRejection::MissingJsonContentType(_) => ApiError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Content-Type must be application/json.",
        ),
        JsonRejection::BytesRejection(_) => ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "request_too_large",
            "Request body is too large.",
        ),
        _ => ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_json",
            "Request body must be a JSON object with a single string \"text\" field.",
        ),
    }
}

struct ValidatedJson<T>(T);

impl<T, S> FromRequest<S> for ValidatedJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(classify_json_rejection)?;
        Ok(ValidatedJson(value))
    }
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyzeRequest {
    text: String,
}

async fn analyze(
    State(state): State<AppState>,
    ValidatedJson(request): ValidatedJson<AnalyzeRequest>,
) -> Result<Response, ApiError> {
    let text = request.text;
    let analyzer = state.analyzer;
    let document = tokio::task::spawn_blocking(move || analyzer.analyze(&text))
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "analysis_task_failed",
                "Analysis could not complete.",
            )
        })?
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "analysis_failed",
                "Analysis failed.",
            )
        })?;

    Ok(Json(document).into_response())
}

pub fn router(analyzer: Arc<Analyzer>) -> Router {
    let state = AppState { analyzer };
    Router::new()
        .route("/api/health", get(health))
        .route("/api/analyze", post(analyze))
        .with_state(state)
}
