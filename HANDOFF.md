# nnj-grammar Handoff

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

The approved design is
`docs/superpowers/specs/2026-07-12-reading-graph-alpha-design.md`. The detailed
desktop implementation plan is
`docs/superpowers/plans/2026-07-12-reading-graph-alpha-stage-a.md`.

## Current Checkpoint

Stage A analysis-core implementation is complete through combined catalog
ranking. The CLI still behaves as before, while the Rust library can now load
embedded Hanabira plus a local Bunpro catalog and classify raw matches as
primary or secondary.

Implemented:

- Reusable Rust library in `src/lib.rs`.
- Catalog provenance and `patterns::load_combined`.
- Raw `matcher::match_candidates` with ranking evidence.
- Legacy-compatible `matcher::match_all` behavior.
- Deterministic `ranking::rank_candidates`.
- Acceptance coverage for `そしてなによりも`:
  - Primary: `そして`
  - Primary: `何より`, spanning `なによりも`
  - Secondary: broad bare-`も` match

The full Stage A plan is:

`docs/superpowers/plans/2026-07-12-reading-graph-alpha-stage-a.md`

## Next Step

Continue with Task 4 from the plan:

1. Create `src/analysis.rs` with schema version 1 records.
2. Create `src/hierarchy.rs` for sentence -> grammar/segment -> token nodes.
3. Add hierarchy tests before implementation.
4. Add `Analyzer` orchestration and inspectable combined JSON.
5. Prioritize the first D3 visualization before JMdict enrichment.

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
git diff --check
```

Focused checkpoint tests:

```bash
cargo test --test analysis_core
cargo test --test ranking
```

Web work requires Node.js 26.x. No Node or D3 files have been created yet.
