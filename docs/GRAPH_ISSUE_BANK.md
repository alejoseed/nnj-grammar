# Graph Issue Bank

A running list of sentences whose analysis graph is wrong, each with a two-line
note on what's wrong. **Do not fix these one at a time** — collect them here and
fix in bulk later, so we don't churn a functional state chasing single cases.

Each entry is a real input plus a terse description. When we do a bulk pass, group
entries by the shared root cause (see the tag in brackets) rather than by
sentence.

Common root-cause tags (see `docs/PIPELINE.md` for the mechanics):
- `[noise]` — too many low-value secondary candidates (§4; needs
  `ambiguity_group`/`fallback` populated + the suppression filter).
- `[junk-primary]` — a weak/wildcard/lone-particle match promoted to primary (§4 ranking).
- `[fragment]` — a real multi-token grammar point split across nodes, or its host orphaned.
- `[clause]` — no clause grouping; flat node row + floating punctuation (§6).
- `[coverage]` — a grammar point exists but no rule matches it as a unit.
- `[gloss-ui]` — meaning is in the payload but not visible in the graph (§9, reading card).
- `[tagging]` — tokenizer/POS mistag (the original missed-grammar cause).
- `[gloss-pick]` — right word, wrong JMdict entry or sense shown (context-free,
  JMdict-order selection in `src/dictionary.rs`).
- `[over-widen]` — family widening is too coarse: a rule's core literal was widened to its
  whole family `one_of`, so semantically-distinct auxiliaries now match each other
  (e.g. a `ておく` rule matches `てる`/`ちゃう`). A correctness regression from the widening work.

---

## Demo wins

Sentences where our deterministic output beats the LLM-backed reference
(hanabira.org grammar-graph). Keep for demos and as regression anchors.

### 国際連合安全保障理事会
- Ours: ONE 単語 node "(United Nations Security Council)" — the full compound is
  a lexicalized JMdict entry, found by longest-match within the bunsetsu after
  the compound cap was removed (schema v3 word leaves).
- Hanabira: split into 3 words, and **non-deterministic** — re-analyzing the
  same input returned a different graph (flat 3-word row vs nested
  国際連合→国際+連合), consistent with an LLM in the loop. Ours is byte-stable
  across runs by construction.

---

## Entries

### 6. イきそうなら、いつでもイってくれていいですからね (function-word gloss senses)
- `[gloss-pick]` — 接続助詞 て glosses as "you said; he said..." (the quotative って
  entry lists て as an alternate reading and its Particle sense passes the
  function-word filter). Hits every て-form in every sentence.
- `[gloss-pick]` — から after です shows "from (time, place)" when the conjunctive
  "because" sense (same entry, sense 2) applies. Sense choice is JMdict-order,
  context-free. Likely batch fix: prefer entries whose primary reading equals the
  surface, and/or bias 接続助詞 to conjunction-flavored senses.

### 1. イきそうなら、いつでもイってくれていいですからね」
> STATUS 2026-08-14: structural complaints below are FIXED (3 bunsetsu, punctuation
> attached, てくれていい assembled — schema v3). Remaining: gloss senses (entry 6).
- `[noise]` `[junk-primary]` `[fragment]` `[clause]` `[gloss-ui]` — 16 tokens spawned
  7 primaries + 4 gaps + 39 secondaries; noise cloud + a lone `て` promoted to primary.
- `そう`+host `行き` split, `なら` conditional buried, `てくれていい` permission fragmented,
  no clause break at `、`, punctuation floats. (Tokenization was correct, incl. katakana イ→行く.)

### 2. オレも、イきましたよ、すっごいイきました
- `[fragment]` `[junk-primary]` `[coverage]` — `も` matches alone as a primary; host `オレ`
  is orphaned in a gap. Want ONE node spanning `オレも` with `オレ` and `も` as token children.
- `よ` (終助詞, sentence-final) is mis-matched as volitional contraction `Verb よう・う ⇒ よ`
  [6-6] and split off from `イきました`; likewise `すっごい` is orphaned from the verb it modifies.

> Design note (from entry 2): a host-attaching particle/aux point should absorb its host
> into the match span and expose host + particle as child tokens, rather than matching the
> bare particle and stranding the host in a gap-segment.

### 3. うん、知ってる……んんっ。だって、わたしの中、ドロドロだもん。本当に、火傷しちゃいそう……ぅぁっ、はぁー……はぁー……
- `[over-widen]` `[fragment]` — `知ってる` splits: `てる` is eaten by a `ておく` rule (its core
  is widened to the whole aspect family `one_of{てる,ちゃう,ちまう,とく,てく,り}`), stranding `知っ`.
  Same bug hits `しちゃいそう` (`ちゃい` → `ておく` too). ROOT CAUSE confirmed in bunpro-local.toml.
- `[clause]` `[noise]` — heavy punctuation: `……` becomes many dot leaves and `ぅぁっ` splits into
  3 `補助記号` tokens; ~1/3 of nodes are symbol/interjection noise floating in gap-segments.

### 4. わたしは
- `[fragment]` — minimal repro of entry 2. `Noun は～` [hanabira-n5-040] matches `token_start=1,
  token_end=1` (は only); `わたし` is stranded as sibling gap `segment-0-0`.
- Root cause confirmed: the noun is `left_context` (`grammar/hanabira/n5.toml:1017`) and
  `src/matcher.rs:29`/`:220` never widen the span past `core`. Want one node `わたしは` with
  `わたし` + `は` as token children — `hierarchy.rs::add_span` already produces that shape if
  the span were `0..1`. See `docs/DEBUGGING_GRAPH_SHAPE.md` for the three fix sites.

### 5. 自動販売機に飲み物を買います
- `[over-widen]` — bunpro-local-70 ましょう matches bare ます (core widened to the ます
  family), so 買います is labeled "Let's, Shall we (Polite volitional)" and the correct
  ます "Polite Verb Endings" match is demoted to secondary (blocked_by match-7-7).
- Same root cause as entry 3's ておく/てる; JLPT tie-break doesn't help because
  the widened core has higher core_specificity (2 vs 1), which outranks it.

> Key finding (from entry 3): the family widening from the current branch is a **correctness
> regression**, not just cosmetic — it makes distinct grammar points (`ておく` vs `ている` vs
> `ちゃう`) mutually match. The bulk fix likely needs per-lemma widening (conjugation only) or a
> much finer family granularity, NOT whole-family `one_of`. Revisit `docs/PIPELINE.md` §10.
