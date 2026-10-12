use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, FromRequest, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use slog::Logger;
use tokio::net::TcpListener;

use crate::analysis::ANALYSIS_SCHEMA_VERSION;
use crate::analyzer::{Analyzer, AnalyzerConfig};
use crate::layouts::{self, LayoutStore, NewSubmission, Rejection, SubmitLayoutRequest};

#[derive(Clone)]
pub struct AppState {
    analyzer: Arc<Analyzer>,
    logger: Logger,
    layouts: Option<LayoutStore>,
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

async fn analyze_text(
    analyzer: Arc<Analyzer>,
    text: String,
) -> Result<crate::analysis::AnalysisDocument, ApiError> {
    tokio::task::spawn_blocking(move || analyzer.analyze(&text))
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
        })
}

async fn analyze(
    State(state): State<AppState>,
    ValidatedJson(request): ValidatedJson<AnalyzeRequest>,
) -> Result<Response, ApiError> {
    validate_text(&request.text)?;
    let document = analyze_text(state.analyzer, request.text).await?;
    Ok(Json(document).into_response())
}

fn client_address(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| peer.map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| "unknown".to_owned())
}

fn rejected(rejection: Rejection) -> ApiError {
    match rejection {
        Rejection::Invalid(message) => {
            ApiError::new(StatusCode::BAD_REQUEST, "invalid_layout", message)
        }
        Rejection::Mismatch(message) => {
            ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "layout_mismatch", message)
        }
    }
}

fn storage_failed() -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "layout_storage_failed",
        "The arrangement could not be saved.",
    )
}

#[derive(Serialize)]
struct SubmitLayoutResponse {
    id: i64,
    status: &'static str,
}

async fn submit_layout(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    ValidatedJson(request): ValidatedJson<SubmitLayoutRequest>,
) -> Result<Response, ApiError> {
    let Some(store) = state.layouts.as_ref() else {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "layouts_unavailable",
            "This server is not collecting arrangements.",
        ));
    };
    let client = client_address(&headers, peer.map(|Extension(ConnectInfo(addr))| addr));
    layouts::check_device_id(&request.device_id).map_err(rejected)?;
    if !store.allow(&client, &request.device_id) {
        return Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many arrangements sent. Try again in a few minutes.",
        ));
    }
    let class_code = layouts::normalize_class_code(&request.class_code).map_err(rejected)?;
    let nickname = layouts::normalize_nickname(request.nickname.as_deref()).map_err(rejected)?;
    let feedback = request.feedback;
    layouts::check_shape(&feedback).map_err(rejected)?;
    let open = store.class_code_open(&class_code).await.map_err(|error| {
        slog::error!(state.logger, "class code lookup failed"; "error" => %error);
        storage_failed()
    })?;
    if !open {
        store.record_bad_code(&client);
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_class_code",
            "That class code is not open. Check the code on the board.",
        ));
    }
    let document =
        analyze_text(state.analyzer.clone(), feedback.sentence.trim().to_owned()).await?;
    layouts::check_graph(&feedback, &document).map_err(rejected)?;
    let flags = layouts::flags(&feedback);
    let id = store
        .insert(NewSubmission {
            class_code: &class_code,
            nickname: nickname.as_deref(),
            client: &client,
            device_id: &request.device_id,
            feedback: &feedback,
            flags,
        })
        .await
        .map_err(|error| {
            slog::error!(state.logger, "layout insert failed"; "error" => %error);
            storage_failed()
        })?;
    slog::info!(state.logger, "layout submitted";
        "id" => id,
        "class_code" => class_code,
        "moved_nodes" => flags.moved_nodes,
        "order_breaks" => flags.order_breaks,
    );
    Ok((
        StatusCode::CREATED,
        Json(SubmitLayoutResponse {
            id,
            status: "pending",
        }),
    )
        .into_response())
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
        "ms" => u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    );
    response
}

pub fn router(analyzer: Arc<Analyzer>) -> Router {
    router_with_logger(analyzer, crate::logging::discard_logger())
}

pub fn router_with_logger(analyzer: Arc<Analyzer>, logger: Logger) -> Router {
    router_with_layouts(analyzer, logger, None)
}

/// [`router_with_logger`] plus `POST /api/layouts`, which answers 503 when
/// `layouts` is `None`.
pub fn router_with_layouts(
    analyzer: Arc<Analyzer>,
    logger: Logger,
    layouts: Option<LayoutStore>,
) -> Router {
    let state = AppState {
        analyzer,
        logger,
        layouts,
    };
    Router::new()
        .route("/api/health", get(health))
        .route("/api/analyze", post(analyze))
        .route("/api/layouts", post(submit_layout))
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
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("server error")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Which local catalog the analyzer ended up with: the copy embedded in the
/// binary, or a `grammar/local` directory on disk that stands in for it.
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
        // Still a full catalog: the local rules are embedded in the binary.
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
