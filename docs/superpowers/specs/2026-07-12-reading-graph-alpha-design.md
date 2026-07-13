# Reading Graph Alpha

## Purpose

Build an offline personal reading assistant that accepts pasted Japanese text
and presents a Hanabira-style hierarchical grammar graph. The graph should help
the reader understand unknown grammar without requiring an LLM or an internet
connection.

The first implementation stage targets the local desktop browser. Native iPhone
work is explicitly deferred until the user gives a separate signal from the
Mac.

## Product Decisions

- Desktop uses a faithful left-to-right Hanabira-style full tree with pan and
  zoom.
- Phone eventually uses an adaptive focus tree with an optional full-map view.
- Both views use the same D3 renderer and versioned analysis JSON.
- Tapping a node opens a layered reading card with meaning, usage information,
  token breakdown, provenance, and optional technical details.
- The first hierarchy is derived from ranked grammar spans rather than a full
  dependency parser.
- Hanabira, personal Bunpro, and local enrichments are analyzed together.
- Strong candidates appear in the primary graph; weaker overlapping candidates
  remain available as secondary possibilities.
- JMdict supplies offline English glosses for ordinary words.
- No local or remote LLM is included in this stage.

## Staged Scope

### Stage A: Implement Now

- Extract the analyzer from the CLI into a reusable Rust library.
- Combine embedded Hanabira with the compiled personal Bunpro catalog.
- Rank, deduplicate, and classify primary and secondary grammar matches.
- Build a deterministic sentence-to-span-to-token hierarchy.
- Import a local JMdict XML file into a compact SQLite database.
- Add token-level dictionary lookup using surface, base form, and reading.
- Define and emit a versioned `AnalysisDocument` JSON response.
- Build the shared TypeScript and D3 reading interface.
- Add a local Rust HTTP server for the desktop browser.
- Add browser-local paste history.
- Preserve the existing CLI outputs.

### Stage B: Deferred Until Mac Signal

- Create an Xcode or SwiftUI project.
- Add UniFFI or any other Swift-to-Rust bridge.
- Package a Rust XCFramework.
- Add `WKWebView`, SwiftData history, signing, or physical-device installation.
- Add iOS-specific resource packaging.

No Stage B files or scaffolding are created during Stage A.

## Architecture

### Rust Analyzer Library

The existing package exposes a library API used by the CLI and local web
server. Tokenization, catalog loading, matching, ranking, dictionary lookup, and
hierarchy construction remain in Rust.

The public Rust boundary is:

```rust
impl Analyzer {
    pub fn analyze(&self, text: &str) -> anyhow::Result<AnalysisDocument>;
}
```

The CLI remains a thin output adapter. The web server serializes the same
document as JSON. A future iOS bridge will call this API without changing its
meaning.

### Catalog Composition

Catalog selection becomes additive:

1. Load embedded Hanabira rules.
2. Load compiled Bunpro rules from `grammar/local/` when present.
3. Include forms already merged from the personal enrichment file.
4. Validate rule IDs across the combined set.

Source names are carried as catalog metadata rather than inferred in the D3
client. Multiple sources may support the same displayed construction.

The existing CLI retains an explicit local-only mode for debugging, while the
desktop reading application defaults to the combined catalog.

### Match Ranking

Every valid matcher result is retained. Candidates are ordered
deterministically by:

1. Non-fallback before fallback.
2. Explicit rule or variant priority.
3. Longer consumed core span.
4. More constrained token predicates.
5. More adjacent context constraints.
6. Fewer wildcard and optional steps.
7. Stable rule and variant IDs.

Candidates are considered in that order. A candidate becomes primary when it
does not overlap an already selected primary candidate. Exact duplicate senses
at the same span are grouped into one display result with multiple provenance
records. Different senses remain distinct alternatives.

Rejected primary candidates are not deleted. They become secondary candidates
with a machine-readable reason such as `contained_by_stronger_match`,
`overlaps_stronger_match`, or `duplicate_sense`.

For `そしてなによりも`, the primary graph contains `そして` and `何よりも`.
The broad `誰か・どこか・誰も・どこも` match on `も` remains inspectable as a
secondary candidate and does not occupy the main graph.

### Span-Based Hierarchy

The first graph is a strict display tree:

```text
sentence
  primary grammar span or unmatched segment
    token
    token
```

Primary grammar spans become children of the sentence root. Contiguous tokens
not owned by a primary span become unmatched segment nodes. Grammar and segment
nodes contain token leaves in source order. Secondary candidates are attached
as metadata to the smallest covering primary or segment node rather than as
tree children.

This hierarchy makes no claim about syntactic dependency or omitted arguments.
It is a stable reading-oriented view of recognized spans. A future parser can
add clause and dependency nodes without changing token or match identities.

## Analysis Document

The web boundary uses versioned JSON rooted in these fields:

```json
{
  "schema_version": 1,
  "input": "そしてなによりも",
  "tokens": [],
  "primary_matches": [],
  "secondary_matches": [],
  "tree": {
    "root_id": "sentence-0",
    "nodes": [],
    "edges": []
  }
}
```

Token records include stable position, byte range, surface, reading, base form,
part of speech, conjugation, and zero or more dictionary glosses. Match records
include rule, variant, sense, meaning, hint or usage note, JLPT level, token
span, source provenance, score inputs, and primary or secondary status.

Tree nodes reference token and match IDs instead of duplicating full records.
IDs are deterministic for the same input and catalog version.

## JMdict

`tools/import_jmdict.py` reads a user-supplied JMdict XML file and writes a
gitignored SQLite database under `grammar/local/`. The importer retains Japanese
spellings, kana readings, English glosses, parts of speech, priority markers,
and entry sequence IDs. It creates indexes for spelling and reading lookup.

At analysis time, each UniDic token is looked up in this order:

1. Base form and reading.
2. Surface and reading.
3. Base form without a reading restriction.
4. Surface without a reading restriction.

Priority-marked entries sort first. The alpha returns at most three distinct
English glosses per token rather than every JMdict sense. Missing entries leave
the gloss list empty and never fail sentence analysis.

JMdict data is not fetched at application runtime.

## Desktop Web Application

The desktop client is a TypeScript application using D3. A local Rust server
binds to loopback, serves static assets, and exposes `POST /api/analyze`.

The page contains:

- A paste field and Analyze action.
- Browser-local recent passage history.
- A full-canvas faithful tree with curved links, sentence and grammar node
  colors, Japanese labels, readings, and concise English glosses.
- Pan, zoom, reset, and fit-to-content controls.
- A layered reading card opened by selecting a node.
- A secondary-candidates disclosure in the reading card.
- A responsive adaptive-focus preview used to develop the future phone layout.

The reading card labels catalog-provided text as meaning or usage. It does not
claim to provide generated contextual interpretation. A deterministic match
section explains which text matched and how it breaks into tokens.

The D3 application renders only the `AnalysisDocument`; it does not duplicate
Japanese matching, ranking, or dictionary logic in TypeScript.

## Local Server

The server listens on `127.0.0.1:7878` by default and does not expose the
analyzer to the network. It rejects empty input and input above 65,536 UTF-8
bytes.
Catalog and dictionary initialization failures stop server startup with a clear
error. Per-request analysis failures return structured JSON without discarding
the text currently displayed in the browser.

Static web assets are packaged with the Rust server for normal use. Development
may use the TypeScript development server with an explicit local API URL.

## History and Privacy

Desktop history is stored in browser local storage and contains input text plus
the analysis timestamp. It retains the 50 most recent distinct passages and has
a visible clear-all action. Analysis remains on the device. The local server
does not log passage contents by default.

Future iPhone history will use SwiftData, but no SwiftData code belongs to Stage
A.

## Testing

Rust tests cover:

- Combined catalog loading and duplicate-ID rejection.
- Deterministic ranking and primary/secondary classification.
- Exact duplicate-sense grouping.
- Span hierarchy construction and source-order preservation.
- JMdict import using a minimal XML fixture.
- Dictionary lookup precedence and bounded gloss output.
- `AnalysisDocument` JSON stability.
- HTTP success and validation responses.
- Existing matcher and Hanabira regression suites.

Reading regressions include at least:

- `言わないが`
- `それは......そうかも」`
- `そしてなによりも`
- The previously tested longer novel sentence

TypeScript tests cover graph-data conversion, focus navigation, reading-card
content, secondary-candidate disclosure, and local history behavior. A browser
smoke check verifies pan, zoom, fit, selection, and responsive mode.

## Completion Criteria

Stage A is complete when:

- One local command starts the desktop reading application.
- Pasted Japanese produces a faithful full tree without network access.
- Hanabira, personal Bunpro, and local enrichments contribute simultaneously.
- Strong specific constructions occupy the graph and weaker overlaps remain
  available as secondary candidates.
- Ordinary recognized words can display bounded JMdict English glosses.
- Recent passages persist locally and can be cleared.
- The existing CLI and regression suite remain operational.
- No SwiftUI, Xcode, UniFFI, XCFramework, or iOS implementation files have been
  introduced.
