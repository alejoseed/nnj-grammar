# nnj-grammar Code Tour

This guide explains the repository from the outside in. Read it before diving
into the matcher or generated grammar files.

## The Mental Model

`nnj-grammar` is a deterministic Japanese analysis pipeline, not a language
model. It combines a morphological tokenizer with executable grammar rules:

```text
Japanese text
  -> UniDic tokens
  -> grammar candidates
  -> ranked primary and secondary matches
  -> sentence/grammar/token hierarchy
  -> schema-v1 JSON
  -> validated D3 reading graph
```

There are currently two paths through the code:

```text
Current CLI path
  text -> tokenize -> one catalog -> match_all -> terminal/DOT/legacy JSON

New reading-app path
  text -> tokenize -> combined catalogs -> match_candidates -> rank_candidates
       -> build_tree -> AnalysisDocument

Current fixture UI path
  committed AnalysisDocument -> validate schema -> validate ordered tree
                             -> derive labels -> render D3 graph
```

The new analysis path is connected by the public `Analyzer`, and the first D3
consumer renders its committed deterministic fixture. The existing CLI does not
call the analyzer yet, and the web page does not call Rust until the loopback API
is implemented.

## Start Here

Read these files in this order:

1. `README.md` for product purpose and commands.
2. `src/lib.rs` for the public module list.
3. `src/cli.rs` and `src/main.rs` for the currently running CLI.
4. `src/tokenizer.rs` for the first transformation of Japanese text.
5. `src/patterns/rule.rs` for the grammar-rule language.
6. `grammar/hanabira/n5.toml` for concrete rule examples.
7. `src/patterns/loader.rs` for catalog loading and validation.
8. `src/matcher.rs` for candidate detection.
9. `src/ranking.rs` for primary/secondary selection.
10. `src/analysis.rs` for the stable future UI contract.
11. `src/hierarchy.rs` for the tree presented to D3.
12. `src/analyzer.rs` for end-to-end orchestration.
13. `tests/analysis_core.rs`, `tests/ranking.rs`, `tests/hierarchy.rs`, and
    `tests/reading_analysis.rs` for
    executable examples of the intended behavior.
14. `web/src/types.ts` and `web/src/graph-model.ts` for the browser boundary.
15. `web/src/graph.ts`, `web/src/app.ts`, and `web/src/main.ts` for rendering.
16. `web/tests/reading-graph.spec.ts` for the real-browser interaction contract.

Do not start by reading all of `matcher.rs`. First understand `PatternRule`,
`Step`, `MatchCandidate`, and the tests that create them.

## Current CLI Flow

### 1. Input: `src/cli.rs`

`Cli` defines:

- Positional Japanese text.
- `--file` input.
- `--output` mode.
- `--grammar-db` for a filesystem catalog.

`Cli::read_text` chooses positional text first, then a file, then piped stdin.

### 2. Orchestration: `src/main.rs`

The binary:

1. Parses the CLI.
2. Reads input.
3. Creates the UniDic tokenizer.
4. Tokenizes once.
5. Returns early for token-only diagnostic modes.
6. Loads either embedded Hanabira or one filesystem catalog.
7. Calls `matcher::match_all`.
8. Sends the result to terminal, DOT, or JSON display code.

Important: the CLI does not yet call `load_combined`, `rank_candidates`,
`build_tree`, or construct `AnalysisDocument`.

### 3. Legacy display: `src/display.rs`

This file owns the current terminal and DOT graph output. It is binary-local and
is not the D3 renderer. The browser renderer under `web/` consumes
`AnalysisDocument` instead.

## Tokenization: `src/tokenizer.rs`

`Tokenizer::new` loads embedded UniDic through Lindera. `Tokenizer::tokenize`
returns one `Token` per morpheme.

Important token fields:

| Field | Meaning |
|---|---|
| `surface` | Text exactly as written |
| `base_form` | Dictionary lemma used for lookup and predicates |
| `reading` | Hiragana reading |
| `pos1` ... `pos4` | UniDic part-of-speech hierarchy |
| `conj_type` | Conjugation class |
| `conj_form` | Current conjugated form |
| `byte_start`, `byte_end` | Original UTF-8 location |
| `position` | Zero-based token index |

Example:

```text
言わないが
  言わ  verb, base 言う
  ない  negative auxiliary
  が    conjunctive particle
```

The grammar matcher works with token positions, not raw character substrings.

## Grammar Rules: `src/patterns/rule.rs`

`PatternRule` describes one grammar point or sense. A rule has metadata and one
or more executable forms.

Key types:

- `PatternRule`: name, JLPT level, meaning, source, and variants.
- `PatternVariant`: one concrete realization of a rule.
- `Step`: one token predicate or bounded wildcard.
- `Boundary`: clause or sentence boundary assertion.
- `CatalogSource`: provenance such as Hanabira or local Bunpro.

A variant has three regions:

```text
left_context | core | right_context
```

Only the core becomes the highlighted match span. Context proves the match but
does not expand the annotation.

Steps can match exact surfaces, POS fields, conjugation, base forms, `one_of`
alternatives, optional tokens, or bounded wildcards. Captures record meaningful
subranges for later UI use.

## Catalogs: `src/patterns/loader.rs`

The loader has three important entry points:

| Function | Behavior |
|---|---|
| `load_embedded()` | Loads generated Hanabira rules compiled into the binary |
| `load_grammar_dir()` | Loads only TOML files under a filesystem directory |
| `load_combined()` | Loads Hanabira plus an optional local Bunpro directory |

Every loaded file is validated. Invalid wildcard bounds, unconstrained steps,
duplicate variants, and duplicate rule IDs fail early.

The reading application will use `load_combined`. The current CLI still chooses
between embedded and filesystem catalogs.

## Matching: `src/matcher.rs`

The matcher answers: "Which rule variants can consume tokens here?"

There are two public entry points:

| Function | Purpose |
|---|---|
| `match_candidates()` | Returns every distinct successful match plus ranking evidence |
| `match_all()` | Preserves the older CLI behavior and resolves candidates immediately |

`MatchCandidate` includes:

- The user-facing `PatternMatch`.
- Rule priority and fallback status.
- Core and context specificity.
- Wildcard and optional-step counts.
- Discovery order used for legacy compatibility.

The matcher does not decide what should dominate the future graph. It reports
evidence; `ranking.rs` makes that presentation decision.

## Ranking: `src/ranking.rs`

`rank_candidates` turns raw candidates into:

```text
RankedMatches
  primary: matches displayed on the main graph
  secondary: weaker overlapping alternatives kept for inspection
```

Ranking prefers:

1. Non-fallback rules.
2. Higher explicit priority.
3. Longer spans.
4. More specific core and context predicates.
5. Fewer wildcard and optional steps.
6. Stable IDs as deterministic tie-breakers.

Exact duplicates from multiple catalogs become one display match with multiple
provenance records. Overlapping weaker matches receive reasons such as
`contained_by_stronger_match`.

For `そしてなによりも`:

```text
Primary:   そして
Primary:   何より, spanning なに + より + も
Secondary: broad bare-も interpretation
```

The secondary result is retained for debugging and ambiguity inspection but
does not clutter the main graph.

## Stable Output: `src/analysis.rs`

This module defines records, not orchestration. `AnalysisDocument` is the
language-neutral contract shared by the current D3 client, the planned server,
and the future phone app.

It contains:

- Schema version.
- Original input.
- Tokens prepared for display and dictionary glosses.
- Primary matches.
- Secondary matches.
- A normalized node/edge tree.

`AnalyzedToken` mirrors `Token` and reserves `glosses` for JMdict. The gloss
list remains empty until dictionary integration is implemented.

Tree records use stable IDs such as:

```text
sentence-0
segment-0-0
match-1-3
secondary-3-3-0
token-2
```

The web renderer follows IDs. It does not perform Japanese-language inference.

## Hierarchy: `src/hierarchy.rs`

`build_tree(tokens, ranked)` creates the first reading-oriented hierarchy:

```text
sentence
  grammar match or unmatched segment
    token
    token
```

The algorithm walks left to right:

1. Add an unmatched segment before the next primary match when needed.
2. Add the primary grammar node.
3. Add token leaves under that node.
4. Add a final unmatched segment when needed.
5. Attach secondary candidates to the smallest covering non-token node.
6. Attach crossing secondary candidates to the sentence root.

This is a display hierarchy, not a dependency parse. It makes no claim about
subjects, omitted arguments, or deep syntax.

## Public Orchestration: `src/analyzer.rs`

`Analyzer` is the first complete entry point for the new reading-app pipeline.
It owns one initialized UniDic tokenizer and the combined grammar catalog so a
consumer does not rebuild expensive state for every sentence.

`AnalyzerConfig` currently accepts:

- `local_grammar_dir`: optional personal Bunpro TOML directory.
- `dictionary_path`: reserved for JMdict integration.

Omitting `local_grammar_dir` loads only the embedded catalog. Supplying it is
an explicit configuration choice, so the path must exist and be a directory;
otherwise initialization returns an error instead of silently falling back to
embedded rules.

Until the dictionary milestone is implemented, supplying `dictionary_path`
returns an explicit initialization error rather than silently pretending that
glosses were loaded.

`Analyzer::analyze` performs exactly one pass through each stage:

```text
tokenize
  -> match_candidates
  -> rank_candidates
  -> convert Token to AnalyzedToken
  -> build_tree
  -> AnalysisDocument
```

The token conversion creates stable IDs such as `token-0` and leaves `glosses`
empty. JMdict will later replace only that enrichment step; it will not change
matching, ranking, hierarchy, or the public document structure.

`tests/fixtures/local-reading.toml` supplies a small deterministic local catalog
for acceptance tests. `tests/fixtures/analysis-soshite.json` freezes one complete
schema version 1 document byte-for-byte. `tests/reading_analysis.rs` verifies
the three focused reading examples, contiguous long-text token byte coverage,
clause boundaries, deterministic serialization, the JSON fixture, and invalid
configuration errors.

## Fixture Web Graph: `web/`

The current browser slice deliberately stops at the committed analyzer fixture.
It proves the complete JSON-to-graph boundary before the loopback API introduces
network orchestration.

### Schema boundary: `web/src/types.ts`

`parseAnalysisDocument` treats fixture JSON as untrusted input. It accepts only
schema version 1 and validates every token, match, secondary candidate, node,
edge, span, and provenance record before the rest of the UI can use it. The
TypeScript records mirror the Rust `AnalysisDocument` contract without adding
Japanese-language inference.

### Ordered topology: `web/src/graph-model.ts`

`buildOrderedTree` validates the normalized graph before turning it into nested
nodes. It rejects duplicate IDs, missing edge references, multiple parents,
cycles, disconnected nodes, and invalid root or span references. Child order is
the original edge order, which keeps layout deterministic.

`buildGraphModel` then derives presentation labels from document records:

- Grammar nodes use the matched surface and English meaning.
- Segment nodes use their covered surface.
- Token nodes use the exact token surface.
- The sentence root remains visually unlabeled.

The model follows stable IDs and references. It does not inspect Japanese text
to decide grammar structure.

### Safe fixture mount: `web/src/app.ts`

`loadAnalysisDocument` fetches and validates JSON. `mountFixtureGraph` keeps the
renderer behind an injected function boundary, so loading and error behavior can
be tested without D3. Failed requests, malformed JSON, and invalid schemas
replace the host with one passage-safe error; no partial input text is exposed.

### Faithful renderer: `web/src/graph.ts`

`renderGraph` reproduces Hanabira's active grammar-tree constants on a
1200-by-800 slate SVG:

- `d3.tree` and `d3.linkHorizontal` produce the left-to-right hierarchy.
- The sentence root is blue; descendants are green.
- Internal labels sit above-left; leaf labels sit to the right.
- Hover and keyboard focus animate to orange over 200 ms.
- Stable DOM IDs, tree roles, labels, and focus targets expose the graph to
  assistive technology and browser tests.

The SVG has separate `viewport` and `plot` groups. Zoom and pan transform only
the viewport, while the plot permanently retains `translate(200,20)`. This
preserves Hanabira's margin without its first-interaction jump. Hover and focus
are tracked independently so either interaction can keep a node emphasized.

`web/src/styles.css` contains only Tailwind's import. Static visual values stay
in Tailwind utilities; D3 owns geometry and interaction state. Text emphasis
uses an inline fill because a Tailwind fill class would otherwise override an
SVG presentation attribute in the browser cascade.

### Browser entry and verification

`web/src/main.ts` locates the semantic host, imports Tailwind, resolves the
committed fixture through Vite, and mounts the graph. `web/vite.config.ts`
allows the repository-root fixture in development and keeps Vitest scoped to
unit tests. `web/src/vite-env.d.ts` supplies Vite's CSS import declarations.

Unit tests cover the schema, topology, labels, safe mount, and renderer. The
Playwright test in `web/tests/reading-graph.spec.ts` launches Chromium and checks
rendering, real CSS hover color, keyboard focus, zoom, pan, and the fixed plot
margin. Its reset screenshot is written under ignored `web/test-results/` for
visual review.

## Data and Generated Files

### `grammar/hanabira/`

Generated embedded catalog: 828 rules plus the regression manifest. Do not
manually fix generated TOML; fix the importer or source normalization and
regenerate it.

### `grammar/compiler/hosts.json`

Maps source labels such as `Noun`, `Verb`, and `Phrase` to UniDic predicates or
bounded wildcards. This keeps source-specific grammar knowledge out of Rust.

### `grammar/local/`

Gitignored personal data:

- Saved Bunpro index.
- Compiled Bunpro TOML.
- Personal enrichment forms.
- Future JMdict SQLite database.

This directory must be recreated or transferred separately on each machine.

### `tools/`

- `import_hanabira.py`: compiles Hanabira formation text into deterministic
  TOML and regression examples.
- `import_bunpro_local.py`: normalizes a personal Bunpro snapshot, merges local
  enrichments, and writes gitignored TOML.
- Python tests protect HTML cleanup, enrichments, furigana alternatives, and
  deterministic import behavior.

## Tests as Documentation

Use tests to understand intended behavior:

| Test | What it demonstrates |
|---|---|
| `tests/library_api.rs` | Public library setup and embedded matching |
| `tests/analysis_core.rs` | Combined catalogs on a real Japanese phrase |
| `tests/ranking.rs` | Primary/secondary rules and determinism |
| `tests/hierarchy.rs` | Exact tree shape and stable IDs |
| `tests/reading_analysis.rs` | Public Analyzer pipeline and reading regressions |
| `src/matcher.rs` tests | Optional tokens, wildcards, boundaries, captures |
| `src/hanabira_regression.rs` | Corpus-wide generated catalog baseline |
| `web/src/types.test.ts` | Runtime schema-v1 validation |
| `web/src/graph-model.test.ts` | Ordered topology and label derivation |
| `web/src/app.test.ts` | Fixture loading and passage-safe errors |
| `web/src/graph.test.ts` | Hanabira structure and node interactions |
| `web/tests/reading-graph.spec.ts` | Chromium rendering, focus, pan, and zoom |

Run one focused test while reading its implementation. It is easier to follow
than reading the entire module first.

## Known Sources of Confusion

- `src/main.rs` still uses the old path even though `Analyzer` can now produce
  complete documents. The web fixture is the first consumer; the next integration
  will be the loopback API. CLI migration remains a separate decision.
- `src/graph/` is dormant unfinished code and is not compiled. Current graph
  output lives in `src/display.rs`; the active D3 graph lives under `web/`.
- Older `grammar/n1` through `grammar/n5` directories are not the embedded
  Hanabira catalog.
- UniDic index comments and the selected base/reading fields in `tokenizer.rs`
  need a documentation consistency pass.
- The detailed implementation plan is a design record. Current progress is
  tracked in `PROJECT_STATUS.md`.

## A Practical First Study Session

1. Run `cargo run --quiet -- --output table "言わないが"`.
2. Read the resulting `Token` fields in `src/tokenizer.rs`.
3. Open one small rule in `grammar/hanabira/n5.toml`.
4. Read `PatternRule`, `PatternVariant`, and `Step`.
5. Read one matcher test using optional or context steps.
6. Read the `そしてなによりも` test in `tests/analysis_core.rs`.
7. Read the ranking test that makes bare `も` secondary.
8. Read the hierarchy test and compare expected IDs with `build_tree`.
9. Read `Analyzer::analyze` and the six end-to-end reading tests.
10. Run the fixture graph and compare `graph-model.ts` with the rendered nodes.

After that sequence, the remaining matcher implementation will have concrete
meaning instead of looking like an isolated recursive algorithm.
