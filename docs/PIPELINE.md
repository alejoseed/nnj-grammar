# nnj-grammar Analysis Pipeline

A step-by-step walkthrough of how a Japanese sentence becomes an analysis
document, written for reasoning about architectural changes. For a
beginner-oriented reading order see [`CODE_TOUR.md`](CODE_TOUR.md); this document
is the "what each stage does, why, and where it could change" reference.

Everything here is **deterministic and offline**. No LLM runs at analysis time;
the analyzer is the source of facts.

---

## 0. The two entry paths

There are two independent ways text flows through the code. Do not confuse them.

```
Legacy CLI path      (src/main.rs, src/cli.rs, src/display.rs)
  text -> tokenize -> ONE catalog -> matcher::match_all -> terminal/DOT/JSON
  (self-contained; does NOT call the Analyzer, ranking, hierarchy, or glosses)

Reading-app path     (src/analyzer.rs and everything below)
  text -> tokenize -> combined catalogs -> match_candidates -> rank_candidates
       -> JMdict glosses -> build_tree -> AnalysisDocument
       -> loopback HTTP API -> web D3 graph
```

The reading-app path is the product. The CLI is a legacy diagnostic tool and is
not wired to the `Analyzer`. Everything below describes the reading-app path.

Top-level orchestration lives in `Analyzer::analyze` (`src/analyzer.rs`), which
runs each stage exactly once per sentence:

```
tokenize -> match_candidates -> rank_candidates
         -> Dictionary::shared().gloss_tokens -> build_tree -> AnalysisDocument
```

---

## 1. Tokenization — `src/tokenizer.rs`

**Input:** raw sentence string. **Output:** `Vec<Token>` (one per morpheme).

Uses **lindera** with **embedded UniDic** (`unidic-mecab-2.1.2`, baked into the
binary via the `lindera-unidic` crate's `embed-unidic` feature). One
deterministic segmentation per sentence — not a ranked list of candidates.

Each `Token` carries the UniDic fields the rest of the pipeline keys on:

| field | meaning |
|---|---|
| `surface` | text exactly as written |
| `pos1`–`pos4` | POS hierarchy (名詞, 動詞, 助詞, 助動詞, …) |
| `conj_type`, `conj_form` | conjugation class / current form |
| `base_form` | **語彙素 (lemma)** — the dictionary form; the key everything downstream matches on |
| `reading` | kana reading (katakana → hiragana) |
| `byte_start`/`byte_end`, `position` | location + zero-based index |

**Key design point:** `base_form` is the lemma, so it is conjugation-invariant
(`いか`/`いき`/`いけ` all → `行く`). This is what makes predicate matching
generalize across conjugation.

**Limitations / change candidates:**
- UniDic tokenizes to **short units**, so compounds (`図書館` → `図書` + `館`) and
  set phrases (`よろしくお願いします`) are split. Handled downstream, imperfectly.
- Colloquial contractions can mis-lemmatize by intuition but follow UniDic
  convention (`ん` → `ず`, not a bug). Pinned by `tests/lexicon_conventions.rs`.
- Mode is `Normal`; there is no longer-unit / named-entity merging.

---

## 2. Grammar catalogs — what rules exist and where they come from

Rules are loaded by `patterns::load_combined` (`src/patterns/loader.rs`), which
merges **two sources** into one pool:

- **Hanabira** — the default catalog, compiled into the binary from
  `grammar/hanabira/` (`RustEmbed`). Generated from the Hanabira content repo.
- **Local Bunpro** — `grammar/local/bunpro-local.toml`, the user's personal,
  gitignored catalog, generated from a saved Bunpro snapshot.

A rule (`PatternRule`, `src/patterns/rule.rs`) is **not** an example sentence; it
is an executable **pattern** over tokens: metadata plus one or more `variant`s,
each with a `core` (the highlighted span) and optional `left_context` /
`right_context` / boundary assertions. A `Step` matches token fields
(`surface`, `pos1`, `pos2`, `conj_form`, `base_form`), a bounded `wildcard`, or a
`one_of` set of alternatives, and may be `optional`.

**Design point:** the runtime is source-neutral. It knows nothing about specific
Japanese grammar; all grammar knowledge lives in the generated TOML and in
`grammar/compiler/hosts.json` (label → UniDic predicate mapping). See §10.

---

## 3. Matching — `src/matcher.rs`

**Input:** tokens + rules. **Output:** every successful match candidate with
ranking evidence.

`match_candidates` asks "which rule variants can consume tokens at each
position?" and returns every distinct match plus evidence (priority, fallback
flag, core/context specificity, wildcard/optional counts, discovery order).
`match_all` is the older, CLI-only variant that resolves immediately.

The matcher **reports evidence; it does not decide** what should win. Multiple
overlapping matches (e.g. `何より` over 3 tokens, `何より` over 2, a bare `も`) all
come out as candidates.

---

## 4. Ranking — `src/ranking.rs`

**Input:** match candidates. **Output:** `RankedMatches { primary, secondary }`.

`rank_candidates` is the **source-blind referee**. It orders overlapping matches
by structural evidence only — never by which catalog a rule came from:

1. non-fallback before fallback
2. higher explicit `priority`
3. longer span
4. more specific core, then context
5. fewer wildcard / optional steps
6. stable IDs as the deterministic tiebreak

The strongest becomes **primary**; weaker overlaps become **secondary** with a
reason (`contained_by_stronger_match`, `overlaps_stronger_match`) and stay
inspectable rather than being deleted.

**Provenance:** each match records which catalog(s) it came from. If the *same*
grammar point exists in both Hanabira and Bunpro, the two are **merged into one**
match citing both sources.

**Limitations / change candidates:**
- Long sentences can produce many low-value secondaries (a particle spawning
  several broad rules). A cleaner filter would suppress secondaries sharing a
  blocker's rule family and `fallback: true` secondaries — but `ambiguity_group`
  and `fallback` are **not populated by the importers today** (see §10), so that
  data doesn't exist yet. Best lever: a *better primary* absorbs the noise.

---

## 5. Dictionary glosses — `src/dictionary.rs`

**Input:** tokens. **Output:** per-token English glosses.

English meanings come from **embedded JMdict** (the `jmdict` crate, baked in at
compile time like UniDic). `Dictionary::shared()` builds the index **once per
process** (lazy `OnceLock`) and every `Analyzer` reuses it, so glosses are always
on and construction stays cheap. The server warms it at startup.

`gloss_tokens(tokens)`:
- Per token: skip function words / punctuation (`助詞`, `助動詞`, `補助記号`,
  `記号`, `空白`); otherwise look up by `base_form`, then `surface`, then
  `reading`, preferring entries whose reading matches the token.
- **Compound pass:** fuse adjacent content tokens whose joined surface is a
  JMdict entry (`図書` + `館` → `図書館` "library") and prepend that gloss to each
  piece.

**Limitations / change candidates:**
- The byte-stable fixture now includes JMdict gloss text, so a `jmdict` version
  bump that changes strings breaks that one test (deliberate snapshot — regen).
- Compound glosses attach to every token in the span; the cleaner home is a
  compound **node** in the tree (same work as the punctuation/clause item).
- Compound matching is greedy and doesn't verify the compound's reading, so a
  rare false positive is possible.

---

## 6. Hierarchy — `src/hierarchy.rs`

**Input:** tokens + ranked matches. **Output:** `AnalysisTree` (nodes + edges).

`build_tree` produces a **flat display hierarchy**, not a dependency parse:

```
sentence
  grammar-match | gap-segment     (segments are only the gaps between matches)
    token
    token
```

It walks left to right, emitting a segment for any gap before the next primary
match, then the match node with its token leaves, then attaches secondary
candidates to the smallest covering non-token node (crossing ones go to the
root). IDs are stable (`sentence-0`, `segment-a-b`, `match-a-b`, `token-n`,
`secondary-a-b-i`).

**Limitations / change candidates:**
- No clause concept. Punctuation (`、`) currently lands as a bare leaf in the
  following gap-segment. Simplest fix (backlog): split gap-segments at
  punctuation and grey it in the renderer (~10 lines, no schema change). Real
  clause detection (て-form / conjunctive particles) is a separate, larger spec.

---

## 7. Stable output — `src/analysis.rs`

`AnalysisDocument` (schema v1, `ANALYSIS_SCHEMA_VERSION`) is the
language-neutral contract shared by the server, the web client, and any future
consumer:

```
schema_version, input, tokens[], primary_matches[], secondary_matches[], tree
```

`AnalyzedToken` mirrors `Token` and adds `glosses`. This module defines records,
not logic. Changing it ripples into `web/src/types.ts` (the TS mirror) — those
two must stay in sync; there is no codegen yet (candidate: `ts-rs`/`typeshare`).

---

## 8. Serving — `src/server.rs` + `src/bin/server.rs`

A thin **Axum** layer over `Analyzer`, bound to `127.0.0.1:7878`
(`nnj-grammar-server`). It marshals HTTP ↔ one `Analyzer::analyze` call; it never
tokenizes/matches/ranks itself.

- `GET /api/health` → `{"status":"ok","schema_version":1}`.
- `POST /api/analyze` accepts `{"text":"…"}` (unknown fields rejected), returns
  the schema-v1 document.
- One `Arc<Analyzer>`; analysis runs on Tokio's blocking pool (CPU-bound).
- Fixed error envelope `{"error":{"code","message"}}` with stable codes;
  validation order is body-size → JSON shape → text validation → analysis.
- **Refuses any non-loopback listener** — privacy by construction.
- `build_analyzer` auto-detects `grammar/local/` (missing → embedded-only;
  dir → combined; non-dir/invalid → startup fails) and warms the JMdict index.

**Design point:** the wire format is **JSON**, not gRPC, deliberately — the
bottleneck is analysis (ms), not the loopback wire (µs), and a browser speaks
JSON natively. Revisit only for a networked/streaming client.

---

## 9. Web rendering — `web/`

Vite + TypeScript + D3 + Tailwind (Node 26). Flow:
`main.ts` (paste box + Analyze) → `app.ts::analyzeText` (`POST /api/analyze`,
proxied by Vite) → `types.ts::parseAnalysisDocument` (validates schema v1 as
untrusted input) → `graph-model.ts::buildGraphModel` (validates topology, derives
labels: grammar node = matched surface + meaning; token node = surface, with a
gloss or non-redundant reading as secondary label) → `graph.ts::renderGraph`
(faithful Hanabira-style left-to-right D3 tree, pan/zoom, accessible focus).

**Limitations / change candidates:**
- Glosses are in the payload but the graph shows structure, not meanings inline;
  the planned "layered reading card" on node click is the home for them.
- The TS types are hand-mirrored from Rust (drift risk; codegen candidate).

---

## 10. The offline build pipeline (how rules are generated)

This is where most architectural leverage lives. Rules are **generated** by
Python importers, then compiled into the binary; the runtime engine never sees
the source content.

### Importers — `tools/import_hanabira.py`, `tools/import_bunpro_local.py`
Both share `Compiler` (the Bunpro importer imports it from the Hanabira one).
They parse human-readable "formations" into `PatternRule` TOML:
- **Host slots** ("Verb", "Noun") → predicates via `hosts.json` (`Compiler.host_step`).
- **Literals** (fixed markers like `わけ に は`) → `Compiler.literal_steps`.
- **Fail-closed:** an entry that can't compile aborts the whole import; an
  unknown host label aborts and names it. Points are never silently dropped.
  (Individual *forms* can be rejected — the Bunpro importer prints a count.)

### `literal_steps` widening (the conjugation fix)
Instead of freezing each literal token's surface, `literal_steps` now:
- conjugating content word (`動詞`/`形容詞`) → `base_form` predicate (matches all
  conjugations);
- closed-class auxiliary → the `one_of` of its grammatical **family** (see
  below);
- everything else → surface (correct for fixed particles).

This is why one `わけにはいかない` rule matches casual `いかない`, polite
`いきません`, and past `いかなかった`.

### `grammar/compiler/hosts.json`
Maps source labels (with spelling aliases) → UniDic predicate sets. Open-class
slot machinery. Small, declarative, human-owned.

### Family completeness — `grammar/compiler/{aux-inventory.json, families.json}`
The provable, fail-closed handling of closed-class auxiliaries:
- `aux-inventory.json` — **machine-owned census** of every closed-class
  auxiliary lemma the embedded UniDic can emit (60 助動詞 + 3 形状詞/助動詞語幹 +
  3 形容詞/非自立可能). Generated by `tools/dump_aux_inventory.py` from the
  dictionary's `lex.csv`; complete because `unk.def` can never assign a
  closed-class POS (the script asserts this).
- `families.json` — **human-owned classification** of each census member into a
  family (negation, aspect, politeness, …) with a register (standard / formal /
  classical / dialect). Widening uses `standard` members by default.
- `tools/test_families.py` — **fail-closed audit**: HALTS naming any census
  member with no family, or any family member absent from the census.
- `tests/lexicon_conventions.rs` — pins the UniDic lemma conventions the
  families rely on, so a dictionary upgrade that shifts them fails loudly.

**Design principle:** the machine owns the *universe* (nothing can be missing);
the human owns the *labels*; every exclusion is a recorded decision.

> **KNOWN DEFECT — over-widening (do not trust the widening blindly).** The audits
> prove *completeness* (every auxiliary is classified), not *meaning preservation*.
> Widening a rule's core literal to its whole family `one_of` collapses
> semantically-distinct grammar points: the `ておく` rule's core is
> `one_of{てる, ちゃう, ちまう, とく, てく, り}`, so it now matches `てる` (progressive,
> in `知ってる`) and `ちゃう` (completive). This is a correctness regression, not
> cosmetic. See `docs/GRAPH_ISSUE_BANK.md` entry 3. Likely fix: per-lemma widening
> (conjugation-invariance only) or much finer family granularity, NOT whole-family
> `one_of`.

**Limitations / change candidates:**
- **Over-widening (above): the biggest one — family `one_of` is too coarse.**
- Per-pattern family overrides (`family:negation[-まい]`) are designed but not
  built; widening is uniform `standard` today.
- Politeness insertion (an optional `ます` between verb and auxiliary) is not
  generated; polite coverage currently depends on the source form or the
  auxiliary being present.
- `ambiguity_group` / `fallback` are not populated by the importers (blocks the
  §4 secondary-noise filter).
- Regenerating `grammar/hanabira/` requires the external Hanabira source; the
  `literal_steps` upgrade applies to Bunpro today and to Hanabira on next regen.
- Inventory re-derivation on a dictionary bump is intentionally **manual**.

---

## 11. Cross-cutting backlog (architectural candidates)

- **Set/fixed phrases** (`よろしくお願いします`): idioms belong in a curated phrase
  lexicon (rule engine) or JMdict `exp` entries, distinct from productive grammar
  patterns like `わけにはいかない`.
- **Colloquial forms** (`って`, contractions): colloquial rule *variants*, not
  text normalization (which would violate "analyzer is the source of facts").
- **Clause structure**: real clause detection would fix punctuation attachment
  and long-sentence grouping in the right shape.
- **Contract codegen**: generate `web/src/types.ts` from the Rust records.
- **Fixture policy**: `analysis-soshite.json` is a deliberate snapshot; regen on
  intentional output changes rather than designing logic around it.
