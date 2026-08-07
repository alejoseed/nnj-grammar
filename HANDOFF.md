# nnj-grammar Handoff

For the canonical progress checklist and exact next action, read
`PROJECT_STATUS.md`. For a guided explanation of the current code, read
`docs/CODE_TOUR.md`.

## Compacted Session Context

The product goal is an offline personal reading assistant for Japanese novels.
The reader pastes a sentence and receives a trustworthy grammar breakdown,
token readings, concise English meanings, and eventually ordinary-word glosses.
The deterministic analyzer is the primary source of facts; an LLM is explicitly
out of scope for Stage A and may only become an optional explanation layer
later.

Approved architecture and UX decisions:

- Rust owns UniDic tokenization, grammar matching, ranking, hierarchy, and
  dictionary lookup.
- Embedded Hanabira, personal local Bunpro, and local enrichments are combined.
- Strong specific matches are primary; weaker overlaps remain inspectable as
  secondary candidates instead of being deleted.
- Desktop uses a faithful Hanabira-style left-to-right D3 tree with pan/zoom.
- A selected node opens a layered reading card with meaning, breakdown,
  provenance, and secondary possibilities.
- A future iPhone view will use the same D3 data in adaptive-focus mode, but no
  SwiftUI, Xcode, UniFFI, or iOS implementation should begin without an explicit
  user signal.
- Offline JMdict glosses are planned after the first visible analysis graph.
- Web tooling must use Node.js 26.x.

Key reading examples and intended behavior:

- `言わないが`: negative `ない` plus contrastive `が`.
- `それは......そうかも」`: topic `は`; `かも` recognized as shortened
  `かもしれない`; standalone `か` becomes a weaker overlapping candidate.
- `そしてなによりも`: primary `そして` and `何より` spanning
  `なによりも`; broad bare-`も` match becomes secondary.
- Long novel sentences should retain clause boundaries and avoid promoted
  Bunpro footnotes or incomplete one-token variants.

The overall approved design is
`docs/superpowers/specs/2026-07-12-reading-graph-alpha-design.md`. The faithful
fixture-graph slice is specified in
`docs/superpowers/specs/2026-07-21-hanabira-faithful-graph-design.md` and its
implementation plan is
`docs/superpowers/plans/2026-07-21-hanabira-faithful-graph.md`.

## Current Checkpoint

Stage A analysis-core implementation is complete through the public `Analyzer`.
The Node.js 26 Vite client now renders the graph from the live loopback API: the
paste box and Analyze button POST `/api/analyze` (Vite proxies `/api` to
`127.0.0.1:7878`) and the current graph is preserved on request failure. Offline
JMdict glosses are live via the embedded `jmdict` crate, and the Bunpro importer
now widens closed-class auxiliaries by grammatical family. The CLI still behaves
as before. `docs/PIPELINE.md` is the current architectural reference.

Implemented:

- Reusable Rust library in `src/lib.rs`.
- Catalog provenance and `patterns::load_combined`.
- Raw `matcher::match_candidates` with ranking evidence.
- Legacy-compatible `matcher::match_all` behavior.
- Deterministic `ranking::rank_candidates`.
- Schema version 1 records in `src/analysis.rs`.
- Sentence -> grammar/segment -> token hierarchy in `src/hierarchy.rs`.
- Stable secondary-candidate IDs and tree attachment references.
- Nullable root spans for empty analysis documents.
- Public `AnalyzerConfig`, `Analyzer::new`, and `Analyzer::analyze` in
  `src/analyzer.rs`.
- Explicit errors for missing configured local grammar directories and the
  not-yet-supported dictionary path.
- Stable `Token` -> `AnalyzedToken` conversion, now populated with JMdict glosses.
- End-to-end reading regressions in `tests/reading_analysis.rs` using
  `tests/fixtures/local-reading.toml`.
- Byte-stable schema regression in `tests/fixtures/analysis-soshite.json`.
- Runtime schema-v1 validation in `web/src/types.ts`.
- Ordered tree validation and deterministic labels in `web/src/graph-model.ts`.
- Passage-safe fixture loading in `web/src/app.ts`.
- Hanabira-faithful D3 rendering in `web/src/graph.ts` with separate viewport
  and plot layers, 200 ms emphasis, accessible focus, pan, and zoom.
- Vite/Tailwind browser entry in `web/src/main.ts` and `web/index.html`.
- Vitest coverage for schema, topology, labels, loading, and rendering.
- Playwright coverage for real-browser rendering, hover, focus, pan, zoom, and
  the stable margin transform.
- Final Fable fidelity review: PASS with no discrepancies.
- Acceptance coverage for `そしてなによりも`:
  - Primary: `そして`
  - Primary: `何より`, spanning `なによりも`
  - Secondary: broad bare-`も` match
- Reusable loopback server module in `src/server.rs` and the
  `nnj-grammar-server` binary in `src/bin/server.rs`.
- Live web wiring: `app.ts::analyzeText` POSTs `/api/analyze`, `main.ts` paste
  box + Analyze button, Vite `/api` proxy, graph preserved on failure.
- Offline JMdict glosses via the embedded `jmdict` crate in `src/dictionary.rs`
  (shared `OnceLock` index, base/surface/reading lookup, compound-fusion pass).
- Provable auxiliary-family widening: `grammar/compiler/aux-inventory.json`
  (machine census), `families.json` (human labels), `tools/test_families.py`
  fail-closed audit, and `tests/lexicon_conventions.rs` UniDic-convention pins.
  CAVEAT: the audits prove *completeness*, not *meaning preservation* — the
  whole-family `one_of` widening is over-broad and currently makes distinct
  grammar points match each other (e.g. `ておく` matches `てる`/`ちゃう`). Known
  defect; see `docs/GRAPH_ISSUE_BANK.md` entry 3 and `docs/PIPELINE.md` §10.

## Local Desktop API Boundary

`src/server.rs` wraps `Analyzer` in an Axum router; `nnj-grammar-server` binds
`127.0.0.1:7878` and serves it with graceful `Ctrl+C` shutdown. The CLI
(`src/main.rs`) is unchanged; this is a separate binary and library module.

Implemented HTTP contract:

- `GET /api/health` -> `200 {"status":"ok","schema_version":1}`.
- `POST /api/analyze` requires `Content-Type: application/json`, accepts
  `{"text":"..."}` (unknown fields rejected), and returns the schema-v1
  `AnalysisDocument` directly, with the input preserved byte-for-byte.
- `Analyzer::analyze` runs on Tokio's blocking pool; the analyzer is built once
  and shared via `Arc`.
- Every error uses the envelope `{"error":{"code","message"}}` with stable
  codes: `invalid_json`/400, `empty_input`/400, `input_too_large`/413 (over
  65,536 UTF-8 bytes), `request_too_large`/413 (raw body over 512 KiB),
  `unsupported_media_type`/415, `not_found`/404, `method_not_allowed`/405,
  `analysis_failed`/500, `analysis_task_failed`/500.
- The serving boundary refuses any non-loopback listener address.
- Startup auto-detects `grammar/local/` relative to the working directory
  (missing -> embedded only; directory -> combined; non-directory or invalid
  catalog -> startup fails). Startup logging reports only the address and catalog
  mode; passage text is never logged.

This wiring is now done (see the web bullets above); the web UI runs against the
live API.

The full Stage A plan is:

`docs/superpowers/plans/2026-07-12-reading-graph-alpha-stage-a.md`

## Next Step

The analyzer, glosses, API, and graph are wired end-to-end. The next slice is
making the glosses (already in the payload) visible and finishing graph UX:

1. Open a layered reading card on node selection (meaning, breakdown,
   provenance, secondary candidates) — the home for the JMdict glosses.
2. Finish the remaining Milestone 4 interactions: reset / fit-to-content,
   secondary-candidate disclosure, keyboard node selection, 50-entry
   browser-local history.
3. Then Milestone 6: embed the built web assets in the Rust server so one binary
   serves the SPA + API from `127.0.0.1:7878`.

Architectural backlog worth folding in (see `docs/PIPELINE.md` §10–11): populate
`ambiguity_group` / `fallback` in the importers to unlock secondary-noise
filtering; clause/punctuation handling; codegen `web/src/types.ts` from the Rust
records; apply `literal_steps` family widening to Hanabira on next regen.

Do not start SwiftUI, Xcode, UniFFI, or other iOS work yet.

## Local Personal Data

`grammar/local/` is intentionally gitignored and must exist on each machine.
It currently contains:

- `bunpro-index.bunpro-local.json`
- `bunpro-local.toml`
- `bunpro-enrichments.bunpro-local.json`

## Verification

Run:

```bash
cargo test --all-targets
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s tools -p 'test_*.py'
mise exec node@26 -- npm --prefix web test
mise exec node@26 -- npm --prefix web run typecheck
mise exec node@26 -- npm --prefix web run build
mise exec node@26 -- npm --prefix web run test:browser
jj status
jj diff --summary
```

Focused checkpoint tests:

```bash
cargo test --test analysis_core
cargo test --test ranking
cargo test --test hierarchy
cargo test --test reading_analysis
```

Web work requires Node.js 26.x. Start the fixture graph with
`mise exec node@26 -- npm --prefix web run dev`.
