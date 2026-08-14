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
- `[over-widen]` — family widening is too coarse: a rule's core literal was widened to its
  whole family `one_of`, so semantically-distinct auxiliaries now match each other
  (e.g. a `ておく` rule matches `てる`/`ちゃう`). A correctness regression from the widening work.

---

## Entries

### 1. イきそうなら、いつでもイってくれていいですからね」
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

> Key finding (from entry 3): the family widening from the current branch is a **correctness
> regression**, not just cosmetic — it makes distinct grammar points (`ておく` vs `ている` vs
> `ちゃう`) mutually match. The bulk fix likely needs per-lemma widening (conjugation only) or a
> much finer family granularity, NOT whole-family `one_of`. Revisit `docs/PIPELINE.md` §10.
