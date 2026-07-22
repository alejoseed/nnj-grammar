# Local Desktop Analysis API Design

## Purpose

Connect the completed Rust `Analyzer` to a stable loopback HTTP contract without
mixing backend work with the next web-interface change. This slice replaces no
fixture and changes no web behavior. It makes live deterministic analysis
available for the following paste-and-Analyze slice.

## Scope

This slice adds:

- A reusable Axum server module in the Rust library.
- A separate `nnj-grammar-server` binary.
- `GET /api/health`.
- `POST /api/analyze`.
- Stable structured JSON errors.
- Exact input validation and bounded request bodies.
- Loopback-only binding.
- Development-time `grammar/local/` auto-detection.
- Graceful shutdown and passage-safe startup/error behavior.
- Unit, router, and loopback TCP tests.

This slice does not add:

- A paste box, Analyze button, or any other web UI.
- A replacement for the committed web fixture.
- CORS or a Vite proxy.
- Static web asset serving.
- Reset, fit-to-content, reading cards, history, or responsive-focus controls.
- JMdict integration.
- LAN access, authentication, or TLS.
- iOS, SwiftUI, Xcode, or a Rust-to-Swift bridge.

## Architecture

The existing `Analyzer` remains the only analysis orchestrator. The HTTP layer
does not tokenize, match, rank, build hierarchy, or infer Japanese grammar.

The library gains a server module responsible for:

- Router construction.
- Shared application state.
- HTTP request and response records.
- Input validation.
- API error conversion.
- Listener validation and serving.
- Local-catalog discovery.

The separate `nnj-grammar-server` binary is responsible only for startup:

1. Resolve `grammar/local/` relative to the process working directory.
2. Build `AnalyzerConfig` from the discovery result.
3. Initialize one `Analyzer`.
4. Build the Axum router with shared analyzer state.
5. Bind `127.0.0.1:7878`.
6. Serve until `Ctrl+C` requests graceful shutdown.

The existing `nnj-grammar` CLI and its legacy output path remain unchanged.

## Shared Analyzer State

The server initializes `Analyzer` once and shares it through `Arc`. Each analyze
handler clones that shared reference and runs `Analyzer::analyze` through
Tokio's blocking pool. Tokenization and matching are CPU-bound and must not run
on Axum's asynchronous I/O workers.

This design avoids reloading embedded UniDic and grammar catalogs for every
request. A dedicated worker queue is intentionally deferred: it would add queue
lifecycle, cancellation, and shutdown behavior without a demonstrated need in a
single-user desktop service. The HTTP contract does not prevent introducing one
later.

## HTTP Contract

### Health

`GET /api/health` returns status `200` and:

```json
{
  "status": "ok",
  "schema_version": 1
}
```

The schema version comes from the same Rust constant used by
`AnalysisDocument`.

### Analyze

`POST /api/analyze` requires `Content-Type: application/json` and accepts:

```json
{
  "text": "そしてなによりも"
}
```

The request record rejects unknown fields. This keeps accidental client/schema
drift visible instead of silently ignoring misspelled or irrelevant values.

On success it returns status `200` and the existing schema-v1
`AnalysisDocument` directly. There is no additional success wrapper. The input
string in the response is byte-for-byte the submitted string.

The endpoint validates the original string as follows:

- `text.trim().is_empty()` is rejected.
- More than 65,536 UTF-8 bytes is rejected.
- Non-empty input at or below 65,536 UTF-8 bytes is accepted for analysis.

The JSON body has a 512 KiB hard limit before extraction so a client cannot
force unbounded allocation with malformed input. This still permits 65,536
decoded UTF-8 bytes when every byte requires JSON escaping. The body limit is
not a replacement for the exact decoded text-field byte check.

## Error Contract

Every API failure returns JSON with this shape:

```json
{
  "error": {
    "code": "empty_input",
    "message": "Text must not be empty."
  }
}
```

The initial stable mappings are:

| Status | Code | Condition |
|---|---|---|
| `400` | `invalid_json` | Malformed JSON, missing `text`, unknown fields, or wrong field types |
| `400` | `empty_input` | Empty or whitespace-only text |
| `413` | `input_too_large` | `text` exceeds 65,536 UTF-8 bytes |
| `413` | `request_too_large` | Raw request exceeds 512 KiB |
| `415` | `unsupported_media_type` | Request content type is not JSON |
| `404` | `not_found` | Request path is not an API route |
| `405` | `method_not_allowed` | API route does not support the request method |
| `500` | `analysis_failed` | `Analyzer::analyze` returns an error |
| `500` | `analysis_task_failed` | The blocking analysis task cannot complete |

Client-facing `500` messages are generic. They do not expose internal error
chains, paths, input fragments, tokens, or serialized analysis data.

Axum extractor rejections must be converted into this contract rather than
leaking Axum's default text responses.

## Binding And Browser Integration

The binary always binds `127.0.0.1:7878`. The reusable serving boundary also
validates the listener's local address and rejects any non-loopback IP. A future
caller therefore cannot accidentally expose the API on `0.0.0.0` or a LAN
interface.

This slice adds no CORS policy. In the following UI slice, Vite will proxy
development `/api` requests to `127.0.0.1:7878`. Packaged assets will eventually
use the same origin as the API.

## Local Grammar Auto-Detection

At startup the binary checks `grammar/local/` relative to the process working
directory:

- If the path does not exist, `AnalyzerConfig.local_grammar_dir` is `None` and
  only embedded Hanabira is loaded.
- If the path is a directory, it is passed explicitly to `AnalyzerConfig`.
- If the path exists but is not a directory, startup fails.
- If the directory contains invalid TOML, duplicate IDs, or another catalog
  error, startup fails rather than silently reverting to embedded rules.

This is a development and desktop-alpha convention. The packaged desktop
milestone will replace working-directory discovery with a platform-specific
personal-data location.

## Privacy And Operational Behavior

The server may report:

- Its fixed listening address.
- Whether embedded-only or combined grammar catalogs were initialized.
- Passage-free startup failures.

The server must not log:

- Request bodies.
- Submitted text.
- Token surfaces or matches.
- `AnalysisDocument` values.
- Error messages containing passage fragments.

No request-body logging middleware is installed. Analysis failures return a
generic API error without logging the submitted value. Port conflicts and
analyzer initialization errors fail startup clearly. `Ctrl+C` stops accepting
new connections and triggers Axum's graceful shutdown path.

## Testing Strategy

Implementation proceeds test-first. Coverage includes:

- Health status and schema version.
- Successful analysis using the real embedded analyzer.
- Exact valid-input preservation.
- Empty and whitespace-only rejection.
- UTF-8 byte-limit checks at, below, and above the boundary without requiring a
  maximum-length tokenizer run for every case.
- Malformed JSON, missing fields, wrong field types, and unsupported content
  types.
- Raw-body hard-limit rejection.
- Stable status codes, error codes, and JSON envelopes.
- Loopback address acceptance and non-loopback rejection.
- Missing, valid, non-directory, and invalid local-catalog discovery cases with
  temporary directories.
- Router tests that do not claim the fixed production port.
- One real TCP smoke test on an ephemeral loopback port.

The existing Rust, Python, TypeScript, production-build, and Playwright suites
remain required regression checks.

## Manual Acceptance

From the repository root:

```bash
cargo run --bin nnj-grammar-server
```

In another terminal:

```bash
curl --fail http://127.0.0.1:7878/api/health

curl --fail \
  -H 'Content-Type: application/json' \
  -d '{"text":"そしてなによりも"}' \
  http://127.0.0.1:7878/api/analyze
```

Acceptance requires a schema-v1 response containing the submitted input, the
expected `そして` and `何より` primary matches, and the existing normalized
tree. Invalid requests must return the documented JSON error shape.

## Completion Criteria

The slice is complete when:

- `nnj-grammar-server` starts on `127.0.0.1:7878` with one reusable analyzer.
- Health and analyze endpoints satisfy the stable contracts above.
- Input and body limits are enforced in UTF-8 bytes.
- Non-loopback listeners are rejected.
- Local grammar discovery follows the explicit four-case behavior.
- Passage text cannot enter server logs or client-facing internal errors.
- Focused API tests and every existing regression suite pass.
- `PROJECT_STATUS.md`, `HANDOFF.md`, and `docs/CODE_TOUR.md` explain the server
  boundary and the next UI integration step.
