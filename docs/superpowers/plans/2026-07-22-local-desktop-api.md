# Local Desktop Analysis API Implementation Plan

**Goal:** Implement `docs/superpowers/specs/2026-07-21-local-desktop-api-design.md`
exactly: a reusable Axum server module in the Rust library, a separate
`nnj-grammar-server` binary, loopback-only binding, structured JSON errors,
exact UTF-8 byte validation, and local-catalog auto-detection, without
changing the existing CLI or any web UI file.

**Architecture:** One new library module, `src/server.rs`, owns router
construction, request/response records, input validation, error conversion,
listener validation, serving, and local-catalog discovery. A new binary,
`src/bin/server.rs` (built as `nnj-grammar-server`), does only startup glue:
discover `grammar/local/`, build one `Analyzer`, build the router, bind
`127.0.0.1:7878`, serve until `Ctrl+C`. `Analyzer::analyze` runs on Tokio's
blocking pool because tokenization and matching are CPU-bound.

**Tech Stack:** Rust 2021 edition, axum 0.8.9, tokio 1.53 (`rt-multi-thread`,
`macros`, `net`, `signal`), existing `anyhow`/`serde`/`serde_json`. Dev-only:
`tower` 0.5 (`util`, for `oneshot`) and `http-body-util` 0.1 (for reading
response bodies in tests). Version control is Jujutsu (`jj`), colocated with
git; commit with `jj commit`, not `git commit`.

## Global Constraints

- Do not modify `src/main.rs`, `src/cli.rs`, `src/display.rs`, or any file
  under `web/`. The CLI and browser fixture path are unaffected by this slice.
- Do not add tracing/logging middleware that could capture request bodies or
  submitted text. No `println!`/`eprintln!` of passage text anywhere.
- All new public items live in `src/server.rs`, re-exported via `pub mod
  server;` in `src/lib.rs`.
- Every task is test-first: write the failing test, confirm red, implement,
  confirm green, then commit with `jj`.
- Use `jj commit -m "<message>"` for every task (author identity already
  configured in this repo; do not pass `--config user.*` overrides).
- Run `cargo fmt --all` before every commit that touches Rust files.

---

## File Map

| File | Responsibility |
|---|---|
| `Cargo.toml` | Add `axum`, `tokio` dependencies; add `tower`, `http-body-util` dev-dependencies; add `[[bin]] nnj-grammar-server` |
| `src/lib.rs` | Export `pub mod server;` |
| `src/server.rs` | Router, state, request/response records, validation, error conversion, loopback check, serving, local-catalog discovery |
| `src/bin/server.rs` | `nnj-grammar-server` binary entry point |
| `tests/server.rs` | Router-level integration tests (`tower::ServiceExt::oneshot`) and one real-TCP smoke test |
| `PROJECT_STATUS.md` | Mark Milestone 3 complete; update next action |
| `HANDOFF.md` | Record the new server boundary and next UI-integration step |
| `docs/CODE_TOUR.md` | Explain the server module and binary |

---

### Task 1: Dependencies And Health Endpoint

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Create: `src/server.rs`
- Create: `tests/server.rs`

**Interfaces:**
- Produces: `nnj_grammar::server::router(analyzer: Arc<Analyzer>) -> axum::Router`
  and `GET /api/health`.

- [ ] **Step 1: Add dependencies**

Add to `Cargo.toml` `[dependencies]`:

```toml
axum = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "signal"] }
```

Add to `Cargo.toml` `[dev-dependencies]`:

```toml
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

Add to `Cargo.toml`, after the existing `[[bin]]` table:

```toml
[[bin]]
name = "nnj-grammar-server"
path = "src/bin/server.rs"
```

Run:

```bash
cargo check --all-targets
```

Expected: fails only because `src/bin/server.rs` does not exist yet. Create a
temporary placeholder `src/bin/server.rs` containing `fn main() {}` so `cargo
check` succeeds; Task 8 replaces it with the real binary.

- [ ] **Step 2: Write the failing health test**

Create `tests/server.rs`:

```rust
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use nnj_grammar::analyzer::{Analyzer, AnalyzerConfig};
use nnj_grammar::server::router;
use tower::ServiceExt;

fn embedded_analyzer() -> Arc<Analyzer> {
    Arc::new(
        Analyzer::new(AnalyzerConfig::default()).expect("embedded analyzer should initialize"),
    )
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
```

- [ ] **Step 3: Confirm the red state**

Run:

```bash
cargo test --test server
```

Expected: fails to compile because `nnj_grammar::server` does not exist.

- [ ] **Step 4: Implement the module skeleton and health handler**

Create `src/server.rs`:

```rust
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
```

In `src/lib.rs`, add:

```rust
pub mod server;
```

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo fmt --all
cargo test --test server
cargo check --all-targets
```

Expected: the health test passes.

```bash
jj status
jj diff --summary
jj commit -m "add loopback server module with health endpoint"
```

---

### Task 2: Analyze Endpoint Happy Path And JSON Error Envelope

**Files:**
- Modify: `src/server.rs`
- Modify: `tests/server.rs`

**Interfaces:**
- Produces: `POST /api/analyze`, `ApiError` (stable `{ "error": { "code",
  "message" } }` envelope), a `ValidatedJson<T>` extractor that converts Axum
  `Json` rejections into that envelope instead of Axum's default plain-text
  responses.

- [ ] **Step 1: Write failing tests for success, malformed JSON, and content type**

Append to `tests/server.rs`:

```rust
fn analyze_request(body: &'static str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/analyze")
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn analyze_returns_schema_v1_document_for_embedded_soshite() {
    let app = router(embedded_analyzer());
    let response = app
        .oneshot(analyze_request(r#"{"text":"そして"}"#))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["schema_version"], 1);
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
    let (status, json) = error_json(
        app.oneshot(analyze_request("not json")).await.unwrap(),
    )
    .await;
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
    let (status, json) = error_json(
        app.oneshot(analyze_request(r#"{}"#)).await.unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "invalid_json");
}

#[tokio::test]
async fn analyze_rejects_wrong_field_type() {
    let app = router(embedded_analyzer());
    let (status, json) = error_json(
        app.oneshot(analyze_request(r#"{"text":5}"#)).await.unwrap(),
    )
    .await;
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
```

- [ ] **Step 2: Confirm the red state**

Run:

```bash
cargo test --test server
```

Expected: fails to compile (no `/api/analyze` route, no such assertions
possible against the current router).

- [ ] **Step 3: Implement the error envelope, validated extractor, and handler**

Replace the contents of `src/server.rs` with:

```rust
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
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo fmt --all
cargo test --test server
cargo check --all-targets
```

Expected: all six tests from Steps 1-3 of Task 1 and Task 2 pass.

```bash
jj status
jj diff --summary
jj commit -m "add analyze endpoint with JSON error envelope"
```

---

### Task 3: Exact UTF-8 Byte Validation

**Files:**
- Modify: `src/server.rs`
- Modify: `tests/server.rs`

**Interfaces:**
- Produces: a pure `fn validate_text(text: &str) -> Result<(), ApiError>`,
  unit-tested directly so the 65,536-byte boundary is checked without running
  the tokenizer three times.

- [ ] **Step 1: Write failing unit tests for the boundary**

Add to `src/server.rs`, in a `#[cfg(test)] mod tests` block at the end of the
file:

```rust
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
```

- [ ] **Step 2: Confirm the red state**

Run:

```bash
cargo test --lib server
```

Expected: fails to compile because `validate_text` does not exist and
`ApiError.code` is private to the module (the test module is inside
`server.rs`, so it can see private fields; that part will compile once the
function exists).

- [ ] **Step 3: Implement validation and wire it into the handler**

In `src/server.rs`, add the constant and function above the `analyze`
handler:

```rust
pub const MAX_TEXT_BYTES: usize = 65_536;

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
```

At the start of the `analyze` function body, before the `spawn_blocking`
call, add:

```rust
validate_text(&request.text)?;
```

- [ ] **Step 4: Add router-level regression tests for the two rejected cases**

Append to `tests/server.rs`:

```rust
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
```

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo fmt --all
cargo test --lib server
cargo test --test server
```

Expected: all unit and integration tests pass.

```bash
jj status
jj diff --summary
jj commit -m "enforce exact UTF-8 byte input limits"
```

---

### Task 4: Raw Request Body Hard Limit

**Files:**
- Modify: `src/server.rs`
- Modify: `tests/server.rs`

**Interfaces:**
- Produces: a 512 KiB hard limit on the raw request body, enforced before
  JSON decoding, mapped to `413 request_too_large`.

- [ ] **Step 1: Write the failing oversized-body test**

Append to `tests/server.rs`:

```rust
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
```

- [ ] **Step 2: Confirm the red state**

Run:

```bash
cargo test --test server analyze_rejects_oversized_raw_body
```

Expected: fails, most likely because the default body limit (if any) does not
match 512 KiB, or because `JsonRejection::BytesRejection` is not yet mapped
correctly. If Axum's default extractor limit already produces a 413 with a
different code, adjust `classify_json_rejection`'s `BytesRejection` arm rather
than the test.

- [ ] **Step 3: Set the explicit 512 KiB body limit**

In `src/server.rs`, add the constant near `MAX_TEXT_BYTES`:

```rust
pub const MAX_REQUEST_BODY_BYTES: usize = 512 * 1024;
```

Add `axum::extract::DefaultBodyLimit` to the imports, and change the router
builder to:

```rust
pub fn router(analyzer: Arc<Analyzer>) -> Router {
    let state = AppState { analyzer };
    Router::new()
        .route("/api/health", get(health))
        .route("/api/analyze", post(analyze))
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .with_state(state)
}
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo fmt --all
cargo test --test server
```

Expected: all tests, including the new oversized-body test, pass. If the
`BytesRejection` arm in `classify_json_rejection` needed adjustment to make
this pass, confirm the earlier malformed-JSON and missing-field tests from
Task 2 still pass unchanged.

```bash
jj status
jj diff --summary
jj commit -m "cap raw request body at 512 KiB"
```

---

### Task 5: Stable 404 And 405 Envelope

**Files:**
- Modify: `src/server.rs`
- Modify: `tests/server.rs`

**Interfaces:**
- Produces: `404 not_found` for unmatched paths and `405
  method_not_allowed` for a wrong method on a matched path, both in the same
  JSON envelope as every other error.

- [ ] **Step 1: Write failing tests**

Append to `tests/server.rs`:

```rust
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
```

- [ ] **Step 2: Confirm the red state**

Run:

```bash
cargo test --test server unmatched_path_returns_json_404 wrong_method_on_known_route_returns_json_405
```

Expected: both fail because Axum's built-in 404/405 responses are empty
plain-text bodies, not the JSON envelope.

- [ ] **Step 3: Add a fallback handler and a response-rewriting layer**

In `src/server.rs`, add:

```rust
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
```

Update the router to add the fallback and the outermost response-rewriting
layer (the last `.layer()` call wraps every earlier one, so it must be added
last to see every response, including the built-in 405):

```rust
pub fn router(analyzer: Arc<Analyzer>) -> Router {
    let state = AppState { analyzer };
    Router::new()
        .route("/api/health", get(health))
        .route("/api/analyze", post(analyze))
        .fallback(not_found)
        .layer(axum::extract::DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(axum::middleware::map_response(rewrite_method_not_allowed))
        .with_state(state)
}
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo fmt --all
cargo test --test server
```

Expected: all tests pass, including every earlier task's tests (the new
layer must not change any existing 2xx/4xx response).

```bash
jj status
jj diff --summary
jj commit -m "return stable JSON envelopes for 404 and 405"
```

---

### Task 6: Loopback-Only Listener Enforcement

**Files:**
- Modify: `src/server.rs`
- Modify: `tests/server.rs`

**Interfaces:**
- Produces: `pub fn ensure_loopback(addr: SocketAddr) -> anyhow::Result<()>`
  and `pub async fn serve(listener: TcpListener, router: Router) ->
  anyhow::Result<()>`, the reusable serving boundary that refuses to run on a
  non-loopback address.

- [ ] **Step 1: Write failing unit tests for the address check**

Add to the `#[cfg(test)] mod tests` block in `src/server.rs`:

```rust
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
```

- [ ] **Step 2: Confirm the red state**

Run:

```bash
cargo test --lib server
```

Expected: fails to compile because `ensure_loopback` does not exist.

- [ ] **Step 3: Implement the address check and the serving boundary**

Add to `src/server.rs`:

```rust
use std::net::SocketAddr;

use anyhow::Context;
use tokio::net::TcpListener;

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
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
```

- [ ] **Step 4: Add one real-TCP smoke test**

Append to `tests/server.rs`:

```rust
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

    let mut stream = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect(addr),
    )
    .await
    .expect("connect within timeout")
    .expect("connect to loopback server");

    stream
        .write_all(
            format!(
                "GET /api/health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
            )
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
```

Note: the second test only re-confirms the pure function through the public
`server` re-export so the integration test file also documents the
requirement; it performs no real bind. Import `ensure_loopback` alongside
`router` at the top of `tests/server.rs`.

- [ ] **Step 5: Verify and commit**

Run:

```bash
cargo fmt --all
cargo test --lib server
cargo test --test server
```

Expected: all unit and integration tests pass, including the real-socket
smoke test.

```bash
jj status
jj diff --summary
jj commit -m "reject non-loopback listeners in the serving boundary"
```

---

### Task 7: Local Grammar Auto-Detection

**Files:**
- Modify: `src/server.rs`
- Modify: `tests/server.rs`

**Interfaces:**
- Produces: `pub enum LocalCatalogMode { EmbeddedOnly, Combined(PathBuf) }`
  and `pub fn build_analyzer(base: &Path) -> anyhow::Result<(Analyzer,
  LocalCatalogMode)>`, implementing the four documented discovery cases.

- [ ] **Step 1: Write failing discovery tests using temporary directories**

Add to the `#[cfg(test)] mod tests` block in `src/server.rs`:

```rust
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
    std::fs::write(
        local.join("empty.toml"),
        "",
    )
    .unwrap();

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
```

- [ ] **Step 2: Confirm the red state**

Run:

```bash
cargo test --lib server
```

Expected: fails to compile because `LocalCatalogMode` and `build_analyzer` do
not exist. Add `tempfile` is already a dev-dependency; no `Cargo.toml` change
needed for this step.

- [ ] **Step 3: Implement discovery and the analyzer-building helper**

Add to `src/server.rs`:

```rust
use std::path::{Path, PathBuf};

use crate::analyzer::AnalyzerConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalCatalogMode {
    EmbeddedOnly,
    Combined(PathBuf),
}

pub fn build_analyzer(base: &Path) -> anyhow::Result<(Analyzer, LocalCatalogMode)> {
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
```

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo fmt --all
cargo test --lib server
cargo test --test server
```

Expected: all four discovery tests and every earlier test pass.

```bash
jj status
jj diff --summary
jj commit -m "auto-detect grammar/local at server startup"
```

---

### Task 8: The `nnj-grammar-server` Binary

**Files:**
- Modify: `src/bin/server.rs` (replace placeholder)

**Interfaces:**
- Produces: a runnable binary that reports its fixed address and catalog mode
  without ever printing passage text, and exits with a clear error on startup
  failure.

- [ ] **Step 1: Replace the placeholder binary**

Replace the contents of `src/bin/server.rs` with:

```rust
use std::env;
use std::sync::Arc;

use anyhow::Context;
use nnj_grammar::server::{build_analyzer, router, serve, LocalCatalogMode};
use tokio::net::TcpListener;

const BIND_ADDR: &str = "127.0.0.1:7878";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cwd = env::current_dir().context("failed to read working directory")?;
    let (analyzer, mode) =
        build_analyzer(&cwd).context("failed to initialize the grammar analyzer")?;

    match mode {
        LocalCatalogMode::EmbeddedOnly => {
            println!("nnj-grammar-server: loaded embedded-only grammar catalog");
        }
        LocalCatalogMode::Combined(path) => {
            println!(
                "nnj-grammar-server: loaded combined grammar catalog ({})",
                path.display()
            );
        }
    }

    let listener = TcpListener::bind(BIND_ADDR)
        .await
        .with_context(|| format!("failed to bind {BIND_ADDR}"))?;
    println!("nnj-grammar-server: listening on http://{BIND_ADDR}");

    serve(listener, router(Arc::new(analyzer))).await
}
```

- [ ] **Step 2: Verify build and run manual acceptance**

Run:

```bash
cargo fmt --all
cargo check --all-targets
cargo run --bin nnj-grammar-server &
sleep 1
curl --fail http://127.0.0.1:7878/api/health
curl --fail -H 'Content-Type: application/json' -d '{"text":"そしてなによりも"}' http://127.0.0.1:7878/api/analyze
kill %1
```

Expected: health returns `{"status":"ok","schema_version":1}`; analyze
returns a schema-v1 document containing `そして` (embedded) as a primary
match, plus `何より` if this machine's gitignored `grammar/local/` directory
is present (it is, on this machine — confirmed via `ls grammar/local/`
showing `bunpro-local.toml`). If `grammar/local/` is absent on a different
machine, only `そして` need appear; this is expected per the discovery
contract, not a regression.

- [ ] **Step 3: Commit**

```bash
jj status
jj diff --summary
jj commit -m "add nnj-grammar-server binary"
```

---

### Task 9: Full Regression Sweep And Documentation

**Files:**
- Modify: `docs/CODE_TOUR.md`
- Modify: `PROJECT_STATUS.md`
- Modify: `HANDOFF.md`

**Interfaces:**
- Consumes: the completed server module and binary.
- Produces: synchronized documentation and a green full regression suite.

- [ ] **Step 1: Explain the server module in the code tour**

Add a `Local Desktop API: src/server.rs` section to `docs/CODE_TOUR.md`,
placed after the "Public Orchestration: `src/analyzer.rs`" section and before
"Fixture Web Graph: `web/`". Cover:

- The router's two routes and the shared `Arc<Analyzer>` state.
- `ValidatedJson` and why Axum's default rejections are intercepted rather
  than returned directly.
- The exact validation order: 512 KiB raw body limit (via
  `DefaultBodyLimit`, surfaced as `request_too_large`), then JSON shape
  (`invalid_json`/`unsupported_media_type`), then `validate_text`
  (`empty_input`/`input_too_large`), then `Analyzer::analyze` on the blocking
  pool.
- `ensure_loopback` and why `serve` refuses non-loopback listeners.
- `build_analyzer`'s four-case `grammar/local/` discovery and
  `LocalCatalogMode`.
- That `nnj-grammar-server` is a separate binary from `nnj-grammar` and does
  not touch `src/main.rs`'s legacy path.

Update the "Start Here" reading order to insert `src/server.rs` and
`src/bin/server.rs` after `src/analyzer.rs`, and add `tests/server.rs` to the
executable-examples list. Update "Known Sources of Confusion" if the CLI/API
relationship needs a one-line clarification.

- [ ] **Step 2: Update project checkpoints**

In `PROJECT_STATUS.md`, mark every Milestone 3 checkbox complete and set the
current next action to the paste/Analyze web-UI wiring slice (Vite `/api`
proxy, fixture-loader replacement, paste field, Analyze button — per
`HANDOFF.md`'s staged plan). Do not check off any Milestone 4 item; this
slice adds no UI code.

In `HANDOFF.md`, record: the server module and binary now exist, the exact
HTTP contract implemented, that the CLI is unchanged, and that the immediate
next step is wiring the web UI to this live API instead of the fixture.

- [ ] **Step 3: Run every regression suite**

Run:

```bash
cargo fmt --all -- --check
cargo test --all-targets
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s tools -p 'test_*.py'
mise exec node@26 -- npm --prefix web test
mise exec node@26 -- npm --prefix web run typecheck
mise exec node@26 -- npm --prefix web run build
mise exec node@26 -- npm --prefix web run test:browser
```

Expected: every suite passes with zero failures or warnings. The web suites
are expected to pass unchanged since no file under `web/` was modified.

- [ ] **Step 4: Inspect and commit documentation**

```bash
jj status
jj diff
jj commit -m "document the local desktop API boundary"
```

- [ ] **Step 5: Move the local bookmark**

```bash
jj bookmark set master -r @-
jj status
jj log -n 12 --no-graph
```

Expected: `master` points at the final documentation commit, and the
working-copy change is limited to the pre-existing, unrelated
`web/src/main.ts` debug line (left untouched by this plan; flag it to the
user separately rather than committing or discarding it here).
