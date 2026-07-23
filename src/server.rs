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

pub const MAX_TEXT_BYTES: usize = 65_536;

pub const MAX_REQUEST_BODY_BYTES: usize = 512 * 1024;

fn validate_text(text: &str) -> Result<(), ApiError> {
    if text.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "empty_input",
            "Text must not be empty.",
        ));
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "input_too_large",
            "Text must not exceed 65,536 UTF-8 bytes.",
        ));
    }
    Ok(())
}

async fn analyze(
    State(state): State<AppState>,
    ValidatedJson(request): ValidatedJson<AnalyzeRequest>,
) -> Result<Response, ApiError> {
    validate_text(&request.text)?;
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
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_whitespace_only_text() {
        assert_eq!(validate_text("").unwrap_err().code, "empty_input");
        assert_eq!(validate_text("   \n\t").unwrap_err().code, "empty_input");
    }

    #[test]
    fn accepts_text_at_the_byte_limit() {
        let text = "a".repeat(65_536);
        assert!(validate_text(&text).is_ok());
    }

    #[test]
    fn rejects_text_above_the_byte_limit() {
        let text = "a".repeat(65_537);
        assert_eq!(validate_text(&text).unwrap_err().code, "input_too_large");
    }

    #[test]
    fn accepts_ordinary_text() {
        assert!(validate_text("そして").is_ok());
    }
}
