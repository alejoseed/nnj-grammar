# Reading Graph Alpha Stage A Implementation Plan

**Goal:** Build the offline desktop reading assistant described in
`docs/superpowers/specs/2026-07-12-reading-graph-alpha-design.md`: one Rust
analysis API combining Hanabira, personal Bunpro, local enrichments, ranked
grammar matches, JMdict glosses, a span hierarchy, and a faithful D3 tree.

**Architecture:** The existing binary becomes a thin adapter over a reusable
Rust library. Rust owns tokenization, catalogs, matching, ranking, dictionary
lookup, and hierarchy construction; a loopback Axum server exposes one
versioned `AnalysisDocument`; a Vite/TypeScript/D3 client renders that document
without Japanese-language logic in JavaScript.

**Tech Stack:** Rust 2021, Lindera/UniDic, Serde, rusqlite, Axum/Tokio,
rust-embed, Python 3 standard library, TypeScript 7, Vite 8, D3 7, Vitest 4,
Playwright 1.61.

## Global Constraints

- Stage A is desktop-only. Do not create SwiftUI, Xcode, UniFFI, XCFramework,
  SwiftData, `WKWebView`, or other iOS implementation files.
- Preserve all current CLI output modes and `cargo run -- ...` behavior.
- Keep the matcher and runtime source-neutral; Japanese-specific knowledge
  belongs in catalog or dictionary data.
- Default server bind is `127.0.0.1:7878`; reject empty text and input larger
  than 65,536 UTF-8 bytes.
- Keep Bunpro, JMdict, and all personal data under gitignored `grammar/local/`.
- Do not fetch grammar or dictionary data at application runtime.
- Do not add an LLM or any network explanation service.
- Web development requires Node.js 26.x; stop before `npm install` when
  `node --version` reports any other major version.
- Follow red-green testing for each task. Do not commit unless the user
  explicitly requests a commit.

## Planned File Structure

```text
src/
  lib.rs                    reusable crate boundary
  analysis.rs               versioned JSON records
  analyzer.rs               orchestration API
  dictionary.rs             read-only JMdict lookup
  hierarchy.rs              span-to-tree conversion
  ranking.rs                primary/secondary selection
  server.rs                 Axum routes and embedded assets
  bin/nnj-grammar-server.rs desktop server entry point
tests/
  library_api.rs
  dictionary.rs
  reading_analysis.rs
  server_api.rs
  support/mod.rs
  fixtures/local-reading.toml
  fixtures/jmdict-mini.xml
  fixtures/analysis-soshite.json
tools/
  import_jmdict.py
  test_import_jmdict.py
web/
  index.html
  package.json
  package-lock.json
  tsconfig.json
  vite.config.ts
  playwright.config.ts
  src/{main,types,api,history,graph,details,styles}.ts
  src/*.test.ts
  tests/reading-graph.spec.ts
  scripts/check-dist.mjs
  dist/                     built assets embedded in server
```

---

### Task 1: Extract the Reusable Rust Library

**Files:**
- Create: `src/lib.rs`
- Create: `tests/library_api.rs`
- Modify: `Cargo.toml`
- Modify: `src/main.rs`

**Produces:** Public modules `matcher`, `patterns`, and `tokenizer`; the current
CLI continues to use the same implementations through the library crate.

- [ ] **Step 1: Add a failing integration test for the public API**

Create `tests/library_api.rs`:

```rust
use nnj_grammar::{matcher, patterns, tokenizer::Tokenizer};

#[test]
fn public_library_tokenizes_and_matches_embedded_rules() {
    let tokenizer = Tokenizer::new().expect("embedded UniDic");
    let tokens = tokenizer.tokenize("そして").expect("tokenization");
    let rules = patterns::load_embedded().expect("embedded Hanabira");
    let matches = matcher::match_all(&tokens, &rules);

    assert!(matches.iter().any(|matched| matched.rule_name.contains("そして")));
}
```

- [ ] **Step 2: Verify the test fails because there is no library crate**

Run: `cargo test --test library_api`

Expected: compilation fails with unresolved crate `nnj_grammar`.

- [ ] **Step 3: Add the library boundary and preserve the default binary**

Add to `Cargo.toml`:

```toml
[package]
default-run = "nnj-grammar"

[lib]
name = "nnj_grammar"
path = "src/lib.rs"
```

Create `src/lib.rs`:

```rust
pub mod matcher;
pub mod patterns;
pub mod tokenizer;

#[cfg(test)]
mod hanabira_regression;
```

Remove `mod matcher`, `mod patterns`, `mod tokenizer`, and
`mod hanabira_regression` from `src/main.rs`. Import those modules from
`nnj_grammar`; retain binary-local `cli` and `display` modules.

- [ ] **Step 4: Verify library and CLI compatibility**

Run:

```bash
cargo test --all-targets
cargo run --quiet -- --output json "そして" >/dev/null
```

Expected: the integration test and the existing 13 Rust tests pass; the CLI
command exits successfully.

---

### Task 2: Add Catalog Provenance and Combined Loading

**Files:**
- Modify: `src/patterns/rule.rs`
- Modify: `src/patterns/loader.rs`
- Modify: `src/patterns/mod.rs`
- Modify: `src/matcher.rs`
- Test: `src/patterns/loader.rs`

**Produces:** `CatalogSource`, `load_combined(local_dir)`, and source provenance
on every `PatternMatch`.

- [ ] **Step 1: Write failing loader tests**

Add `tempfile = "3.27.0"` under `[dev-dependencies]`. Add tests using
`tempfile::tempdir()` that write one valid local TOML rule and
assert:

```rust
let rules = load_combined(Some(local.path())).expect("combined catalog");
assert_eq!(rules.iter().filter(|rule| rule.source.id == "hanabira").count(), 828);
assert!(rules.iter().any(|rule| {
    rule.id == "bunpro-local-test" && rule.source.id == "bunpro-local"
}));
```

Add a second test whose local fixture reuses an embedded rule ID and assert the
error contains `duplicate pattern id`.

- [ ] **Step 2: Run the focused tests and observe missing API failures**

Run: `cargo test patterns::loader::tests --lib`

Expected: compilation fails because `source` and `load_combined` do not exist.

- [ ] **Step 3: Add source metadata and additive loading**

Add this non-TOML metadata type in `src/patterns/rule.rs`:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct CatalogSource {
    pub id: String,
    pub label: String,
}
```

Add `#[serde(skip)] pub source: CatalogSource` to `PatternRule`. Refactor the
loader so parsing accepts a source value and assigns it after deserialization.
Keep `load_grammar_dir` as local-only behavior, and add:

```rust
pub fn load_combined(local_dir: Option<&Path>) -> Result<Vec<PatternRule>>;
```

`load_combined` loads embedded Hanabira first, then every TOML under the local
directory as source `{ id: "bunpro-local", label: "Bunpro local" }`, and runs
one duplicate-ID validation over the merged vector. A missing optional local
directory is allowed; a supplied existing directory with invalid TOML is not.

Copy `rule.source` into `PatternMatch` with `#[serde(skip_serializing)]` so
downstream Rust code never infers a source from IDs while the existing CLI JSON
shape remains byte-compatible. `AnalysisDocument` later serializes provenance
through its own records.

- [ ] **Step 4: Verify combined loading**

Run:

```bash
cargo test patterns::loader::tests --lib
cargo test --all-targets
```

Expected: combined-source and duplicate-ID tests pass; all existing tests stay
green.

---

### Task 3: Preserve Match Evidence and Rank Primary/Secondary Results

**Files:**
- Create: `src/ranking.rs`
- Modify: `src/lib.rs`
- Modify: `src/matcher.rs`
- Test: `src/matcher.rs`
- Test: `src/ranking.rs`

**Produces:** `matcher::match_candidates`, compatibility `match_all`, and
`ranking::rank_candidates`.

- [ ] **Step 1: Add failing matcher evidence tests**

Add a matcher test that constructs a two-token specific rule and a one-token
fallback rule and checks raw results retain both plus their evidence:

```rust
let candidates = match_candidates(&tokens, &rules);
assert_eq!(candidates.len(), 2);
assert!(candidates.iter().any(|candidate| candidate.fallback));
assert!(candidates.iter().any(|candidate| candidate.core_specificity > 0));
```

Also retain the existing `match_all` assertions to prove its same-span
deduplication behavior remains compatible.

- [ ] **Step 2: Expose raw candidates without changing `match_all`**

Rename the private candidate record to public `MatchCandidate`, derive `Clone`
and `Serialize`, and include:

```rust
pub matched: PatternMatch,
pub fallback: bool,
pub priority: i32,
pub core_specificity: usize,
pub context_specificity: usize,
pub wildcard_steps: usize,
pub optional_steps: usize,
#[serde(skip)] pub discovery_order: usize,
```

Implement `match_candidates(tokens, rules)` by removing the current
first-success `break` and retaining every distinct successful core end for each
rule/variant/start. Assign `discovery_order` when the existing traversal appends
each result. Deduplicate only records equal in rule, variant, span, captures,
core/context specificity, wildcard count, and optional count; never discard a
path with different ranking evidence. To preserve current CLI behavior,
`match_all` keeps the lowest `discovery_order` for each
`(rule, variant, start)` before the existing same-span resolver. Add tests for
both traversal rules: optional token consumption is tried before skipping, and
wildcard counts are tried from `min` upward. Raw candidates contain every
completion while `match_all` returns the same first-discovered completion as
HEAD.

- [ ] **Step 3: Add failing ranking tests**

In `src/ranking.rs`, test these independent behaviors with synthetic
`MatchCandidate` fixtures:

1. Catalog rule `何より`, matching surface `なによりも` at `[1,3]`, is
   primary and a broad `も` at `[3,3]` is secondary with
   `contained_by_stronger_match`.
2. Two non-overlapping candidates are both primary and source ordered.
3. Exact span/name/meaning duplicates become one display match with two
   provenance entries.
4. Same-span different meanings remain separate, with the weaker one secondary.
5. Reversing candidate input produces byte-for-byte identical JSON.

- [ ] **Step 4: Implement deterministic ranking**

Define serializable records:

```rust
pub struct MatchScore { /* every ordering input */ }
pub struct MatchProvenance { /* source, rule_id, variant_id */ }
pub struct DisplayMatch { /* stable id, grammar fields, span, score, provenance */ }
pub struct SecondaryMatch { pub matched: DisplayMatch, pub reason: SecondaryReason, pub blocked_by: Option<String> }
pub struct RankedMatches { pub primary: Vec<DisplayMatch>, pub secondary: Vec<SecondaryMatch> }
```

Implement the specification order exactly: non-fallback, priority, longer core,
core specificity, context specificity, fewer wildcard/optional steps, then IDs.
Group exact duplicates by `(span, normalized name, normalized meaning)` before
overlap selection. Use inclusive-span containment for
`contained_by_stronger_match`; use `overlaps_stronger_match` otherwise.

- [ ] **Step 5: Verify evidence and ranking**

Run:

```bash
cargo test matcher::tests --lib
cargo test ranking::tests --lib
cargo test --all-targets
```

Expected: all ranking scenarios pass and old matcher behavior remains green.

---

### Task 4: Define `AnalysisDocument` and Build the Span Hierarchy

**Files:**
- Create: `src/analysis.rs`
- Create: `src/hierarchy.rs`
- Modify: `src/lib.rs`
- Test: `src/hierarchy.rs`

**Produces:** Stable schema version 1 records and `build_tree`.

- [ ] **Step 1: Add failing source-order and attachment tests**

Build four synthetic tokens and ranked results for catalog rule `何より`
matching surface `なによりも` at `[1,3]`, plus a secondary `も` `[3,3]`.
Assert:

```rust
let tree = build_tree(&tokens, &ranked);
assert_eq!(tree.root_id, "sentence-0");
assert_eq!(tree.children_of("sentence-0"), ["segment-0-0", "match-1-3"]);
assert_eq!(tree.children_of("match-1-3"), ["token-1", "token-2", "token-3"]);
assert_eq!(tree.node("match-1-3").secondary_match_ids, ["secondary-3-3-0"]);
```

Add an unmatched-token test proving adjacent uncovered tokens become one segment
node and punctuation/source order are preserved.

- [ ] **Step 2: Define versioned analysis records**

In `src/analysis.rs`, define:

```rust
pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;

pub struct AnalysisDocument {
    pub schema_version: u32,
    pub input: String,
    pub tokens: Vec<AnalyzedToken>,
    pub primary_matches: Vec<DisplayMatch>,
    pub secondary_matches: Vec<SecondaryMatch>,
    pub tree: AnalysisTree,
}
```

`AnalyzedToken` flattens the existing `Token` and adds
`glosses: Vec<DictionaryGloss>`. Define `DictionaryGloss` here with
`entry_seq: i64`, `gloss: String`, and `pos: Vec<String>` so the schema compiles
before the dictionary implementation and Task 5 returns the same public type.
Define `TreeNodeKind::{Sentence, Grammar, Segment, Token}`, `TreeNode`,
`TreeEdge`, and `AnalysisTree`. Keep IDs based only on token spans and stable
indexes.

- [ ] **Step 3: Implement hierarchy construction**

Create the root, then walk tokens left-to-right. Emit primary grammar nodes at
their starts and group all contiguous uncovered tokens into segment nodes.
Grammar and segment nodes own token leaves. Attach each secondary candidate to
the smallest covering primary grammar or segment node, explicitly excluding
token leaves; fall back to the sentence root.

- [ ] **Step 4: Verify hierarchy and serialization**

Run:

```bash
cargo test hierarchy::tests --lib
cargo test --all-targets
```

Expected: hierarchy tests pass with deterministic node and edge order.

---

### Task 5: Import and Query JMdict Offline

**Files:**
- Create: `tools/import_jmdict.py`
- Create: `tools/test_import_jmdict.py`
- Create: `tests/fixtures/jmdict-mini.xml`
- Create: `tests/support/mod.rs`
- Create: `tests/dictionary.rs`
- Create: `src/dictionary.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`
- Modify: `.gitignore`

**Produces:** `grammar/local/jmdict.sqlite3` schema,
`Dictionary::lookup(spelling, reading)`, and `Dictionary::lookup_token`
returning at most three glosses.

- [ ] **Step 1: Add a minimal JMdict fixture and failing importer test**

The fixture must include:

- One priority kanji entry with a kana reading and two English glosses.
- One kana-only entry.
- One reading restricted to a specific spelling.
- One non-English gloss that must be ignored.
- One later sense that omits `<pos>` and therefore inherits the previous
  sense's POS list.

The Python test imports into `tempfile.TemporaryDirectory()`, opens SQLite, and
asserts the exact `entry`, `form`, and `gloss` rows and indexes.

Run: `python3 -m unittest discover -s tools -p 'test_import_jmdict.py'`

Expected: import fails because `import_jmdict` does not exist.

- [ ] **Step 2: Implement streaming XML-to-SQLite import**

Expose positional CLI arguments `SOURCE_XML DESTINATION_DB`. Use only
`xml.etree.ElementTree.iterparse` and `sqlite3`. Create:

```sql
CREATE TABLE entry(seq INTEGER PRIMARY KEY, priority INTEGER NOT NULL);
CREATE TABLE form(entry_seq INTEGER, form_order INTEGER, spelling TEXT, reading TEXT, priority INTEGER NOT NULL);
CREATE TABLE gloss(entry_seq INTEGER, sense_order INTEGER, gloss_order INTEGER, gloss TEXT);
CREATE TABLE sense_pos(entry_seq INTEGER, sense_order INTEGER, pos_order INTEGER, pos TEXT);
CREATE INDEX form_spelling_reading ON form(spelling, reading);
CREATE INDEX form_reading ON form(reading);
CREATE INDEX gloss_entry_order ON gloss(entry_seq, sense_order, gloss_order);
CREATE INDEX sense_pos_entry_order ON sense_pos(entry_seq, sense_order, pos_order);
```

Expand valid spelling/reading pairs while honoring `re_restr`; use the reading
as spelling for kana-only entries. Keep English or unspecified-language glosses
only. Preserve source order explicitly in `form_order`, `sense_order`,
`gloss_order`, and `pos_order`. Within one entry, a sense with no `<pos>` inherits
the preceding non-empty POS list. Write to a temporary database and atomically
replace the destination after success.

- [ ] **Step 3: Verify the importer**

Run: `python3 -m unittest discover -s tools -p 'test_import_jmdict.py'`

Expected: all fixture rows and indexes pass.

- [ ] **Step 4: Add failing Rust lookup tests**

Add `rusqlite = { version = "0.40.1", features = ["bundled"] }`. In
`tests/support/mod.rs`, add `jmdict_fixture_db()` to create the same SQLite
schema and deterministic test rows in a `tempfile::TempDir`. Create integration
test `tests/dictionary.rs` with `mod support;` and test exact lookup:

```rust
assert_eq!(dictionary.lookup("降る", "ふる")?[0].gloss, "to fall (rain)");
assert_eq!(dictionary.lookup("かな", "かな")?.len(), 1);
assert!(dictionary.lookup("不存在", "ふそんざい")?.is_empty());
assert!(dictionary.lookup("多義語", "たぎご")?.len() <= 3);
```

Then construct a token whose base and surface map to conflicting fixture
entries and assert `lookup_token` returns base+reading results before
surface+reading and spelling-only fallbacks.

- [ ] **Step 5: Implement bounded read-only lookup**

Open SQLite read-only and assign lookup tiers in this order: base+reading,
surface+reading, base-only, surface-only. Within a tier order by form priority
descending, entry priority descending, entry sequence ascending, sense order,
gloss order, then gloss text. Join `sense_pos` in `pos_order`, deduplicate exact
gloss strings without disturbing that order, and stop at three.
`lookup_token(&Token)` performs the tier sequence and missing rows return an
empty vector.

- [ ] **Step 6: Verify Python and Rust dictionary layers**

Run:

```bash
python3 -m unittest discover -s tools -p 'test_import_jmdict.py'
cargo test --test dictionary
```

Expected: both suites pass. Ensure `/grammar/local/` still ignores the generated
database; add `*.sqlite3-*` ignores only if SQLite sidecars appear outside that
directory.

---

### Task 6: Implement the Public `Analyzer` and Reading Regressions

**Files:**
- Create: `src/analyzer.rs`
- Create: `tests/reading_analysis.rs`
- Create: `tests/fixtures/local-reading.toml`
- Create: `tests/fixtures/analysis-soshite.json`
- Modify: `src/lib.rs`

**Produces:** `Analyzer::new`, `Analyzer::analyze`, and complete schema version 1
documents.

- [ ] **Step 1: Create a deterministic local grammar fixture**

Include only rules needed to represent personal-catalog behavior in tests:
u-verb negative `ない`, contrastive `が`, topic `は`, standalone question
particle `か`, shortened `かも` under `かもしれない`, `何より(も)`, and the
broad WH-word-plus-`も` candidate. Use source-neutral TOML predicates; do not
read gitignored `grammar/local/` in automated tests.

- [ ] **Step 2: Add failing end-to-end analysis tests**

Define:

```rust
let (dictionary_dir, dictionary_path) = support::jmdict_fixture_db()?;
let analyzer = Analyzer::new(AnalyzerConfig {
    local_grammar_dir: Some(fixture_dir()),
    dictionary_path: Some(dictionary_path),
})?;
```

Keep `dictionary_dir` alive through the assertions so the temporary database is
not deleted while SQLite is open.

Assert:

- `言わないが` retains negative and contrastive `が` coverage.
- `それは......そうかも」` has primary topic `は` and `かもしれない` spanning
  `か も`; standalone `か` is secondary.
- `そしてなによりも` has primary Hanabira `そして` and local `何より`; the
  latter matches surface `なによりも`, and the broad bare `も` candidate is
  secondary.
- The long novel sentence returns valid schema version 1 without crossing
  clause boundaries.
- Serializing `そしてなによりも` matches
  `tests/fixtures/analysis-soshite.json` byte-for-byte.

- [ ] **Step 3: Implement analyzer configuration and orchestration**

Use:

```rust
pub struct AnalyzerConfig {
    pub local_grammar_dir: Option<PathBuf>,
    pub dictionary_path: Option<PathBuf>,
}

pub struct Analyzer { /* tokenizer, combined rules, optional dictionary */ }

impl Analyzer {
    pub fn new(config: AnalyzerConfig) -> anyhow::Result<Self>;
    pub fn analyze(&self, text: &str) -> anyhow::Result<AnalysisDocument>;
}
```

`analyze` tokenizes once, calls `match_candidates`, ranks, enriches each token
with dictionary glosses, builds the hierarchy, and returns schema version 1.
Dictionary absence is allowed for CLI/tests; an invalid supplied dictionary is
an initialization error.

- [ ] **Step 4: Verify full deterministic analysis**

Run:

```bash
cargo test --test reading_analysis
cargo test --all-targets
```

Expected: all four reading regressions and the JSON fixture pass.

---

### Task 7: Add the Loopback Analysis Server

**Files:**
- Create: `src/server.rs`
- Create: `src/bin/nnj-grammar-server.rs`
- Create: `tests/server_api.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`

**Produces:** `POST /api/analyze`, `GET /api/health`, and server CLI defaults.

- [ ] **Step 1: Add server dependencies and failing route tests**

Add compatible dependencies:

```toml
axum = "0.8.9"
tokio = { version = "1.52.3", features = ["macros", "rt-multi-thread", "signal", "sync"] }
tower-http = { version = "0.7.0", features = ["cors"] }
mime_guess = "2.0.5"
webbrowser = "1.2.1"

[dev-dependencies]
tower = { version = "0.5.3", features = ["util"] }
http-body-util = "0.1.3"
```

Tests call the router and public server configuration validation directly and
assert:

- `GET /api/health` returns 200.
- Valid Japanese returns 200 and schema version 1.
- Empty or whitespace-only input returns 400.
- 65,537 UTF-8 bytes returns 413.
- Malformed JSON returns 400 with the same structured `ApiError` shape.
- A server configuration using `0.0.0.0`, `::`, or any non-loopback address is
  rejected before binding.

- [ ] **Step 2: Implement routes and structured errors**

Define `AnalyzeRequest { text }`, `ApiError { code, message }`, and an Axum
state containing `Arc<tokio::sync::Mutex<Analyzer>>`. Do not log request text.
Accept `Result<Json<AnalyzeRequest>, JsonRejection>` in the handler and convert
extractor failures into `ApiError`; do not expose Axum's default text rejection.
Define public `ServerConfig { bind: SocketAddr, open_browser: bool }` with
`validate()` requiring `bind.ip().is_loopback()`, plus `app(analyzer) -> Router`
and `serve(config, analyzer)`. `serve` binds first, then calls
`webbrowser::open` only when `open_browser` is true; failure to open a browser is
reported without terminating the bound server. Tests call
`ServerConfig::validate` with `open_browser: false`; the binary calls `serve`,
so test and production use the same validation. Allow CORS only for
the Vite development origins `http://127.0.0.1:5173` and
`http://localhost:5173`; packaged same-origin use needs no CORS.

- [ ] **Step 3: Add the server binary**

The binary accepts:

```text
--bind 127.0.0.1:7878
--grammar-dir grammar/local
--dictionary grammar/local/jmdict.sqlite3
--open
```

Parse `--bind` as `SocketAddr`, require `ip().is_loopback()`, and fail with a
clear error before initializing or binding when it is not loopback.
The grammar directory is optional when absent; the dictionary path supplied by
the server defaults is required and startup errors explain how to run
`tools/import_jmdict.py`. `--open` uses `webbrowser` after binding.

- [ ] **Step 4: Verify API behavior**

Run:

```bash
cargo test --test server_api
cargo test --all-targets
```

Expected: all status and payload tests pass; no static UI is expected yet.

---

### Task 8: Scaffold the TypeScript Reading Client and Local History

**Files:**
- Create: `web/package.json`
- Create: `web/package-lock.json`
- Create: `web/tsconfig.json`
- Create: `web/vite.config.ts`
- Create: `web/index.html`
- Create: `web/src/types.ts`
- Create: `web/src/api.ts`
- Create: `web/src/history.ts`
- Create: `web/src/history.test.ts`
- Create: `web/src/details.ts`
- Create: `web/src/details.test.ts`
- Create: `web/src/main.ts`
- Create: `web/src/styles.css`

**Produces:** Typed API consumption, paste/analyze shell, 50-entry distinct
history, and the layered reading card.

- [ ] **Step 1: Initialize exact web dependencies**

Create a private package with scripts `dev`, `build`, `test`, `test:browser`,
and `typecheck`. Declare `"engines": { "node": ">=26 <27" }` and use:

```json
"dependencies": { "d3": "^7.9.0" },
"devDependencies": {
  "@playwright/test": "^1.61.1",
  "@types/d3": "^7.4.3",
  "@types/node": "^26.1.1",
  "jsdom": "^29.1.1",
  "typescript": "^7.0.2",
  "vite": "^8.1.4",
  "vitest": "^4.1.10"
}
```

First run `node --version` and require Node 26.x. The current shell reports Node
21, so install/select Node 26 before continuing. Then run:

```bash
npm --prefix web install
npm --prefix web exec playwright install chromium
```

- [ ] **Step 2: Mirror the schema in TypeScript**

Define discriminated `TreeNodeKind`, `AnalysisDocument`, analyzed token,
display match, secondary match, tree node, and edge interfaces. `api.ts` posts
`{ text }` to `/api/analyze`, validates `schema_version === 1`, and surfaces
structured API errors. Configure Vite's development server to proxy `/api` to
`http://127.0.0.1:7878`; production keeps the same relative URL.

- [ ] **Step 3: Write failing history tests**

Test that history:

- Deduplicates by exact text and moves reused text to the front.
- Keeps the newest timestamp.
- Trims to 50 entries.
- Ignores empty text.
- Clears the configured local-storage key.
- The visible `Clear history` control clears both storage and the rendered
  history list.

- [ ] **Step 4: Implement history and verify**

Use one namespaced key, `nnj-grammar.history.v1`, and inject `Storage` into the
history helper so tests use an isolated in-memory implementation.

Run: `npm --prefix web test -- history.test.ts`

Expected: history tests pass.

- [ ] **Step 5: Write failing layered-card tests**

Given a selected grammar node, assert the card renders name, JLPT level,
catalog meaning/hint, matched token breakdown, provenance, and an initially
collapsed secondary-candidates disclosure. Assert it never labels generated
text as “in this sentence.”

- [ ] **Step 6: Implement the app shell and reading card**

Build semantic DOM for paste input, Analyze button, history drawer, graph
container, and details panel. Keep all Japanese inference out of TypeScript;
the card follows IDs into `AnalysisDocument` records. If a later request fails,
show the error beside the input but leave the previously rendered analysis and
selected card intact.

Run:

```bash
npm --prefix web test
npm --prefix web run typecheck
```

Expected: history and card tests pass with no TypeScript errors.

---

### Task 9: Implement the Faithful D3 Tree and Responsive Focus View

**Files:**
- Create: `web/src/graph.ts`
- Create: `web/src/graph.test.ts`
- Modify: `web/src/main.ts`
- Modify: `web/src/styles.css`
- Create: `web/playwright.config.ts`
- Create: `web/tests/reading-graph.spec.ts`

**Produces:** Desktop full tree, pan/zoom/fit/reset, node selection, and the
approved adaptive-focus behavior at narrow widths.

- [ ] **Step 1: Write failing graph conversion tests**

Convert flat nodes/edges to a D3 hierarchy and assert:

- Exactly one root is required.
- Child order follows edge order and source token order.
- Missing references and cycles throw clear errors.
- Grammar, segment, and token node data survive conversion unchanged.

- [ ] **Step 2: Implement tree conversion and faithful layout**

Use `d3.hierarchy` plus `d3.tree().nodeSize(...)`, swapping coordinates for a
left-to-right tree. Render curved horizontal links, blue sentence root, green
grammar/segment nodes, Japanese labels, readings, and concise English glosses.
Do not encode rule names or Japanese surfaces in renderer branches.

- [ ] **Step 3: Add interactions**

Use `d3.zoom` for pan and zoom. Implement controls for zoom in/out, fit to
content, and reset. Single-click selects a node and opens the layered card.
Keyboard focus and Enter/Space selection must work for SVG nodes.

- [ ] **Step 4: Add the responsive focus mode**

At narrow widths, initially show sentence children and the selected branch;
provide a `Full map` toggle that restores the complete pan-and-zoom tree. This
is shared D3 behavior only, not iOS scaffolding.

- [ ] **Step 5: Add browser smoke coverage**

Playwright resolves the fixture with
`new URL("../../tests/fixtures/analysis-soshite.json", import.meta.url)` and
intercepts `/api/analyze`. Verify Analyze renders nodes, selecting catalog rule
`何より` (matched surface `なによりも`) opens the card, drag changes the pan
transform, zoom changes scale, fit-to-content frames all nodes, reset restores
the initial transform, history survives reload, clear-all removes history, an
API error preserves the current graph, and a narrow viewport focuses the
selected branch and shows `Full map`. Tab to an SVG node and verify both Enter
and Space select it and update the reading card.

Run:

```bash
npm --prefix web test
npm --prefix web run typecheck
npm --prefix web run test:browser
```

Expected: unit and browser tests pass.

---

### Task 10: Embed the Web Build and Document One-Command Startup

**Files:**
- Modify: `src/server.rs`
- Modify: `src/bin/nnj-grammar-server.rs`
- Modify: `README.md`
- Create: `web/scripts/check-dist.mjs`
- Create/update: `web/dist/**`
- Test: `tests/server_api.rs`

**Produces:** A self-contained desktop server binary serving the compiled D3
client and API from the same loopback origin.

- [ ] **Step 1: Build the client and add failing static-route tests**

Run: `npm --prefix web run build`

Add tests asserting `/` returns HTML, a real built JS asset returns the expected
MIME type, and unknown non-API routes return the SPA entry while unknown
`/api/*` routes return JSON 404.

- [ ] **Step 2: Embed `web/dist` with `rust-embed`**

Add a `WebAssets` embed in `src/server.rs`, resolve `index.html` for SPA
fallback, use `mime_guess` for content types, and set immutable caching only on
hashed assets. API routes must take precedence over static fallback.

- [ ] **Step 3: Document local data setup and startup**

Add README commands:

```bash
python3 tools/import_jmdict.py /path/to/JMdict_e.xml grammar/local/jmdict.sqlite3
npm --prefix web install
npm --prefix web run build
cargo run --bin nnj-grammar-server -- --open
```

Document that Hanabira is embedded, Bunpro is loaded from `grammar/local/`, all
analysis is local, history is browser-local, and SwiftUI/iPhone work is not part
of Stage A.

- [ ] **Step 4: Verify packaged startup and asset freshness**

Create `web/scripts/check-dist.mjs`. It uses `mkdtemp`, runs Vite with the
temporary directory as `outDir`, recursively compares file names and bytes with
`web/dist`, and removes the temporary directory in `finally`. Add package script
`check:dist` for it. This catches stale or untracked output without relying on
Git's treatment of untracked files.

Run:

```bash
npm --prefix web run build
npm --prefix web run check:dist
cargo test --test server_api
cargo test --all-targets
cargo build --release --bin nnj-grammar-server
```

Start `target/release/nnj-grammar-server` against fixture/local data and request
`/` plus `/api/health`; this verifies release-mode `rust-embed` serves assets
without reading `web/dist` at runtime. Expected: rebuilt assets are identical,
server tests pass, and both release requests return 200.

---

### Task 11: Stage A Completion Verification

**Files:**
- Verify only; modify implementation files only to resolve discovered defects.

- [ ] **Step 1: Run every automated check**

```bash
python3 -m unittest discover -s tools -p 'test_*.py'
cargo test --all-targets
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
npm --prefix web test
npm --prefix web run typecheck
npm --prefix web run test:browser
npm --prefix web run build
npm --prefix web run check:dist
cargo build --release --bin nnj-grammar-server
git diff --check
```

Expected: all commands exit successfully with no warnings denied by Clippy.

- [ ] **Step 2: Verify the four reading regressions manually through the API**

Start `cargo run --bin nnj-grammar-server`, then submit:

```text
言わないが
それは......そうかも」
そしてなによりも
今さらながらに思うんだけどさ......相手の顔色窺って様子を見てるだけっていうのは、相手を一番困らせるんだと思う
```

Expected: the first three satisfy their automated primary/secondary assertions;
the long sentence returns a valid graph without a server error.

- [ ] **Step 3: Verify privacy, portability, and scope**

Confirm:

- `git status --ignored` shows Bunpro and JMdict under ignored
  `grammar/local/`.
- The server listens only on `127.0.0.1:7878`.
- Passage text does not appear in server logs.
- `cargo run --quiet -- --output graph "そして"` still works.
- No Swift, Xcode project, UniFFI, XCFramework, SwiftData, or iOS implementation
  files exist in the diff.

- [ ] **Step 4: Inspect the desktop acceptance flow**

Paste `そしてなによりも`, confirm the faithful left-to-right tree shows
primary `そして` and catalog rule `何より` spanning surface `なによりも`,
select `何より` to inspect its layered reading card, and expand secondary
candidates to see the broad bare-`も` match without placing it on the main
graph.
