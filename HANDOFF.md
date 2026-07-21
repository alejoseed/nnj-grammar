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
The first visible D3 slice is also complete: a Node.js 26 Vite client validates
the committed schema-v1 fixture and renders it as a faithful Hanabira graph. The
CLI still behaves as before, and the browser remains fixture-driven until the
loopback API is implemented.

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
- Stable `Token` -> `AnalyzedToken` conversion with empty glosses until JMdict.
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

The full Stage A plan is:

`docs/superpowers/plans/2026-07-12-reading-graph-alpha-stage-a.md`

## Next Step

Connect the completed analyzer and graph through the local desktop API:

1. Add the loopback analysis endpoint around `Analyzer`.
2. Replace the development fixture load with `POST /api/analyze` after the
   endpoint contract is tested.
3. Add the paste-and-Analyze shell and preserve the current graph on request
   failures.
4. Return to offline JMdict after the graph path is visible end-to-end.

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
