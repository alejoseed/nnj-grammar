# nnj-grammar Project Status

This is the canonical progress tracker. Update it whenever a task is completed
or priorities change. `HANDOFF.md` contains session context; the detailed Stage
A implementation plan remains in
`docs/superpowers/plans/2026-07-12-reading-graph-alpha-stage-a.md`.

## Goal

Build an offline personal Japanese reading assistant that turns pasted novel
text into a trustworthy grammar hierarchy with readings, concise meanings,
ordinary-word glosses, and a faithful interactive graph.

## Current Checkpoint

Stage A is complete through the public `Analyzer`. The analyzer now connects
tokenization, combined catalogs, matching, ranking, token records, and hierarchy
into one deterministic `AnalysisDocument`.

The existing CLI still uses the legacy direct path. No server, D3 client,
JMdict integration, or iOS implementation exists yet.

## Progress Legend

- `[x]` complete and verified
- `[ ]` not started or incomplete
- `[~]` intentionally deferred
- `[!]` blocked or requires a decision

## Milestone 1: Deterministic Analysis Core

- [x] Expose tokenizer and grammar engine through a reusable Rust library.
- [x] Load embedded Hanabira with catalog provenance.
- [x] Add optional local Bunpro catalog loading.
- [x] Preserve raw matcher candidates and ranking evidence.
- [x] Rank primary and secondary matches deterministically.
- [x] Group duplicate displays while retaining provenance.
- [x] Define `AnalysisDocument` schema version 1.
- [x] Build sentence -> grammar/segment -> token hierarchy.
- [x] Give secondary candidates stable IDs and tree attachments.
- [x] Cover `そしてなによりも` ranking and hierarchy behavior.

## Milestone 2: First End-to-End Analysis Document

Current next action: write failing loopback API tests against the public
`Analyzer`.

- [x] Create `src/analyzer.rs`.
- [x] Add `AnalyzerConfig` for an optional local grammar directory.
- [x] Reject an explicitly configured local grammar path that is not a directory.
- [x] Convert `Token` into `AnalyzedToken` with stable token IDs.
- [x] Orchestrate tokenize -> candidates -> ranking -> hierarchy.
- [x] Return schema version 1 `AnalysisDocument`.
- [x] Add deterministic JSON fixture coverage.
- [x] Cover `言わないが`.
- [x] Cover `それは......そうかも」`.
- [x] Cover `そしてなによりも`.
- [x] Cover the longer novel sentence from the handoff.

## Milestone 3: Local Desktop API

- [ ] Add loopback-only Axum server.
- [ ] Add `POST /api/analyze`.
- [ ] Add `GET /api/health`.
- [ ] Reject empty input and input above 65,536 UTF-8 bytes.
- [ ] Return structured JSON errors.
- [ ] Refuse non-loopback bind addresses.
- [ ] Add `nnj-grammar-server` binary.
- [ ] Verify passage text is not logged.

## Milestone 4: First Visible D3 Reading Graph

Web tooling requires Node.js 26.x.

- [ ] Create Vite, TypeScript, and D3 project under `web/`.
- [ ] Mirror `AnalysisDocument` schema in TypeScript.
- [ ] Add paste and Analyze workflow.
- [ ] Render faithful left-to-right hierarchy.
- [ ] Add curved links and sentence/grammar/token node styles.
- [ ] Add pan, zoom, reset, and fit-to-content.
- [ ] Open layered reading card on node selection.
- [ ] Show secondary candidates in a collapsed disclosure.
- [ ] Add keyboard-accessible node selection.
- [ ] Add responsive adaptive-focus mode and Full map toggle.
- [ ] Add 50-entry distinct browser-local history.
- [ ] Preserve the current graph when a later request fails.

## Milestone 5: Offline JMdict Glosses

This milestone follows the first visible graph so dictionary work does not block
UI feedback.

- [ ] Add a minimal JMdict XML test fixture.
- [ ] Implement streaming XML-to-SQLite importer.
- [ ] Store spellings, readings, senses, POS, glosses, and priority.
- [ ] Add read-only Rust dictionary lookup.
- [ ] Resolve base+reading before surface and spelling-only fallbacks.
- [ ] Return at most three distinct English glosses per token.
- [ ] Populate `AnalyzedToken.glosses`.
- [ ] Keep JMdict data under gitignored `grammar/local/`.

## Milestone 6: Packaged Desktop Alpha

- [ ] Embed built web assets in the Rust server.
- [ ] Serve the SPA and API from `127.0.0.1:7878`.
- [ ] Add one-command startup with optional browser opening.
- [ ] Verify release binary serves assets without source files.
- [ ] Document Bunpro and JMdict local-data setup.
- [ ] Run all Rust, Python, TypeScript, and browser checks.
- [ ] Manually verify all four reading regression sentences.

## Milestone 7: Native iPhone Alpha

Do not start this milestone without an explicit user signal from the Mac.

- [~] Create SwiftUI/Xcode project.
- [~] Package Rust as an XCFramework.
- [~] Add a Swift-to-Rust bridge.
- [~] Host shared D3 assets in `WKWebView`.
- [~] Use adaptive-focus mode by default.
- [~] Add Full map faithful tree mode.
- [~] Store paste history with SwiftData on iOS 17+.
- [~] Install directly through Xcode for personal alpha testing.

## Maintenance and Known Debt

- [ ] Resolve UniDic documentation mismatch around base-form and reading indices.
- [ ] Decide whether to remove dormant uncompiled `src/graph/` code.
- [ ] Align README Hanabira percentage wording with enforced test thresholds.
- [x] Replace Git commands in the active handoff and Stage A plans with
      Jujutsu equivalents.
- [ ] Keep private `sessions.json` and all personal catalog data untracked.
- [ ] Decide when the legacy CLI should switch to `Analyzer` output.

## Verification Commands

```bash
cargo test --all-targets
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
python3 -m unittest discover -s tools -p 'test_*.py'
jj status
jj diff --summary
```

Focused current-core checks:

```bash
cargo test --test analysis_core
cargo test --test ranking
cargo test --test hierarchy
```

## How to Maintain This Tracker

1. Keep exactly one sentence under `Current next action` describing the next
   test or implementation step.
2. Mark a checkbox complete only after its focused and regression checks pass.
3. Add newly discovered work to the relevant milestone instead of relying on
   chat history.
4. Update `HANDOFF.md` at the end of a substantial session.
5. Keep detailed design discussion in the Stage A plan; keep this file concise
   and operational.
