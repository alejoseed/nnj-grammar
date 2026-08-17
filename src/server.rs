use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use slog::Logger;
use tokio::net::TcpListener;

use crate::analysis::ANALYSIS_SCHEMA_VERSION;
use crate::analyzer::{Analyzer, AnalyzerConfig};

#[derive(Clone)]
pub struct AppState {
    analyzer: Arc<Analyzer>,
    logger: Logger,
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

async fn not_found() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        "The requested resource does not exist.",
    )
}

async fn rewrite_method_not_allowed(response: Response) -> Response {
    if response.status() == StatusCode::METHOD_NOT_ALLOWED {
        return ApiError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "This method is not supported for this route.",
        )
        .into_response();
    }
    response
}

async fn log_request(
    State(state): State<AppState>,
    request: Request,
    next: axum::middleware::Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let started = std::time::Instant::now();
    let response = next.run(request).await;
    slog::info!(state.logger, "request";
        "method" => %method,
        "path" => path,
        "status" => response.status().as_u16(),
        "ms" => started.elapsed().as_millis() as u64,
    );
    response
}

pub fn router(analyzer: Arc<Analyzer>) -> Router {
    router_with_logger(analyzer, crate::logging::discard_logger())
}

pub fn router_with_logger(analyzer: Arc<Analyzer>, logger: Logger) -> Router {
    let state = AppState { analyzer, logger };
    Router::new()
        .route("/api/health", get(health))
        .route("/api/analyze", post(analyze))
        .fallback(not_found)
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(axum::middleware::map_response(rewrite_method_not_allowed))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            log_request,
        ))
        .with_state(state)
}

pub fn ensure_loopback(addr: SocketAddr) -> anyhow::Result<()> {
    anyhow::ensure!(
        addr.ip().is_loopback(),
        "refusing to serve on non-loopback address: {}",
        addr.ip()
    );
    Ok(())
}

pub async fn serve(listener: TcpListener, router: Router) -> anyhow::Result<()> {
    let local_addr = listener
        .local_addr()
        .context("failed to read listener address")?;
    ensure_loopback(local_addr)?;
    serve_unrestricted(listener, router).await
}

/// [`serve`] without the loopback guard. For deployments where the caller has
/// explicitly chosen a non-loopback bind — e.g. inside a container, where
/// loopback is unreachable and the host's port mapping is the boundary.
pub async fn serve_unrestricted(listener: TcpListener, router: Router) -> anyhow::Result<()> {
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCatalogMode {
    EmbeddedOnly,
    Combined(PathBuf),
}

pub fn build_analyzer(base: &Path) -> anyhow::Result<(Analyzer, LocalCatalogMode)> {
    // Build the embedded JMdict index eagerly so the first request isn't slow.
    crate::dictionary::Dictionary::shared();
    let candidate = base.join("grammar/local");
    if !candidate.exists() {
        let analyzer = Analyzer::new(AnalyzerConfig::default())
            .context("failed to initialize embedded-only analyzer")?;
        return Ok((analyzer, LocalCatalogMode::EmbeddedOnly));
    }
    anyhow::ensure!(
        candidate.is_dir(),
        "grammar/local exists but is not a directory: {}",
        candidate.display()
    );
    let analyzer = Analyzer::new(AnalyzerConfig {
        local_grammar_dir: Some(candidate.clone()),
        dictionary_path: None,
    })
    .context("failed to initialize combined analyzer")?;
    Ok((analyzer, LocalCatalogMode::Combined(candidate)))
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

    #[test]
    fn accepts_ipv4_and_ipv6_loopback_addresses() {
        assert!(ensure_loopback("127.0.0.1:7878".parse().unwrap()).is_ok());
        assert!(ensure_loopback("[::1]:7878".parse().unwrap()).is_ok());
    }

    #[test]
    fn rejects_non_loopback_addresses() {
        assert!(ensure_loopback("0.0.0.0:7878".parse().unwrap()).is_err());
        assert!(ensure_loopback("10.0.0.5:7878".parse().unwrap()).is_err());
        assert!(ensure_loopback("192.168.1.20:7878".parse().unwrap()).is_err());
    }

    use tempfile::tempdir;

    #[test]
    fn embedded_only_when_grammar_local_is_missing() {
        let base = tempdir().unwrap();
        let (_, mode) = build_analyzer(base.path()).expect("embedded analyzer");
        assert!(matches!(mode, LocalCatalogMode::EmbeddedOnly));
    }

    #[test]
    fn combined_when_grammar_local_is_a_valid_directory() {
        let base = tempdir().unwrap();
        let local = base.path().join("grammar/local");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("empty.toml"), "patterns = []\n").unwrap();

        let (_, mode) = build_analyzer(base.path()).expect("combined analyzer");
        assert!(matches!(mode, LocalCatalogMode::Combined(path) if path == local));
    }

    #[test]
    fn fails_when_grammar_local_is_not_a_directory() {
        let base = tempdir().unwrap();
        std::fs::create_dir_all(base.path().join("grammar")).unwrap();
        std::fs::write(base.path().join("grammar/local"), "not a directory").unwrap();

        assert!(build_analyzer(base.path()).is_err());
    }

    #[test]
    fn fails_when_grammar_local_has_an_invalid_catalog() {
        let base = tempdir().unwrap();
        let local = base.path().join("grammar/local");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("broken.toml"), "not valid toml =====").unwrap();

        assert!(build_analyzer(base.path()).is_err());
    }
}
