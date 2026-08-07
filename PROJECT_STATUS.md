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

Stage A is complete through the public `Analyzer`, and the web client now runs
against the live loopback API. The analyzer produces one deterministic
`AnalysisDocument`; the browser validates that contract and renders a faithful
Hanabira-style hierarchy with keyboard focus, pan, and zoom.

The loopback desktop API is complete: `nnj-grammar-server` serves `Analyzer`
over `127.0.0.1:7878` with health, analyze, structured JSON errors, exact input
limits, loopback-only binding, and `grammar/local/` auto-detection. The web UI is
wired to it: `main.ts` has a paste box and Analyze button, `app.ts::analyzeText`
POSTs `/api/analyze`, and Vite proxies `/api` to `127.0.0.1:7878`.

Offline JMdict glosses are also live, via the embedded `jmdict` crate rather than
the originally planned XML-to-SQLite importer (see Milestone 5). The existing CLI
still uses the legacy direct path. The iOS implementation does not exist yet.

`docs/PIPELINE.md` is the current architectural source of truth for the
reading-app path and the offline build pipeline.

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

The web UI is wired to the live loopback API (Vite `/api` proxy, live
`analyzeText`, paste field and Analyze button).

Current next action: add the layered reading card on node selection so the
JMdict glosses already in the payload become visible (Milestone 4).

- [x] Add loopback-only Axum server.
- [x] Add `POST /api/analyze`.
- [x] Add `GET /api/health`.
- [x] Reject empty input and input above 65,536 UTF-8 bytes.
- [x] Return structured JSON errors.
- [x] Refuse non-loopback bind addresses.
- [x] Add `nnj-grammar-server` binary.
- [x] Verify passage text is not logged.

## Milestone 4: First Visible D3 Reading Graph

Web tooling requires Node.js 26.x.

- [x] Create Vite, TypeScript, Tailwind, and D3 project under `web/`.
- [x] Mirror and validate `AnalysisDocument` schema in TypeScript.
- [x] Load the deterministic schema-v1 fixture through Vite.
- [x] Add paste and Analyze workflow.
- [x] Render faithful left-to-right hierarchy.
- [x] Add curved links and sentence/grammar/token node styles.
- [x] Add pan and zoom with a stable plot-margin transform.
- [ ] Add reset and fit-to-content.
- [ ] Open layered reading card on node selection.
- [ ] Show secondary candidates in a collapsed disclosure.
- [ ] Add keyboard-accessible node selection.
- [ ] Add responsive adaptive-focus mode and Full map toggle.
- [ ] Add 50-entry distinct browser-local history.
- [x] Preserve the current graph when a later request fails.

## Milestone 5: Offline JMdict Glosses

Delivered via the embedded `jmdict` crate (compile-time, baked into the binary
like UniDic) instead of the originally planned XML-to-SQLite importer. The
importer, on-disk fixture, and `grammar/local/` storage below are therefore
obsolete and intentionally not built. See `docs/PIPELINE.md` §5.

- [~] Add a minimal JMdict XML test fixture. (obsolete: no XML import path)
- [~] Implement streaming XML-to-SQLite importer. (obsolete: embedded crate)
- [~] Store spellings, readings, senses, POS, glosses, and priority. (crate owns)
- [x] Add read-only Rust dictionary lookup. (`src/dictionary.rs`)
- [x] Resolve base+reading before surface and spelling-only fallbacks.
- [x] Return English glosses per token, capped at `MAX_GLOSSES_PER_TOKEN` (6;
      the original spec said 3), with function words skipped.
- [x] Populate `AnalyzedToken.glosses`.
- [~] Keep JMdict data under gitignored `grammar/local/`. (obsolete: embedded)

Notable additions beyond the original spec:

- [x] Compound-fusion pass (`図書` + `館` -> `図書館`), prepended to each piece.
- [x] Shared per-process index via `OnceLock` (`Dictionary::shared`); server
      warms it at startup.

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

- [!] Family widening is **over-broad** — widening a rule core to its whole family
      `one_of` makes distinct grammar points match each other (e.g. `ておく` matches
      `てる`/`ちゃう`). Correctness regression from the family-completeness branch; the
      audits only prove completeness, not meaning preservation. See
      `docs/GRAPH_ISSUE_BANK.md` entry 3 and `docs/PIPELINE.md` §10.
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
mise exec node@26 -- npm --prefix web test
mise exec node@26 -- npm --prefix web run typecheck
mise exec node@26 -- npm --prefix web run build
mise exec node@26 -- npm --prefix web run test:browser
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
