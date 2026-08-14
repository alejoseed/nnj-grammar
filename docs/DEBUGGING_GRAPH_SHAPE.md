# How to debug a wrong-shaped graph node

Method for "why did the graph split/merge this wrong", worked through on `わたしは`.

## The pipeline, in the order to suspect it

```
text
 └─ src/tokenizer.rs        Token { surface, pos1..4, conj_form, base_form, position }
     └─ src/matcher.rs      PatternMatch { rule_id, token_start, token_end }   <- spans decided HERE
         └─ src/ranking.rs  primary[] vs secondary[]                          <- which match wins
             └─ src/hierarchy.rs::build_tree   TreeNode + TreeEdge            <- graph shape
                 └─ src/server.rs             JSON (AnalysisDocument)
                     └─ web/src/graph-model.ts  GraphNode (labels only)
                         └─ web/src/graph.ts    SVG
```

**Node identity and parent/child come from `src/hierarchy.rs` alone.** The web
side never invents, merges, or reparents nodes — `graph-model.ts` is a 1:1
conversion that only computes labels. So a wrong *shape* is always upstream of
the browser. Don't start in `web/`.

## The diagnostic ladder

Run these in order and stop at the first surprise.

1. **Is tokenization right?**
   ```
   ./target/debug/nnj-grammar --output table "わたしは"
   ```
   Wrong POS here means every later stage is guessing. (`--output raw` dumps all
   29 UniDic fields.)

2. **What span does each match claim?** This is the one that matters most.
   ```
   ./target/debug/nnj-grammar --output json "わたしは"
   ```
   Read `matches[].token_start` / `token_end`. Compare to the tokens you *think*
   the grammar point covers.

3. **What tree came out?** The CLI's `json` is the legacy shape and has no tree.
   Use the server for the real `AnalysisDocument`:
   ```
   ./target/debug/nnj-grammar-server &
   curl -s localhost:7878/api/analyze -H 'content-type: application/json' \
     -d '{"text":"わたしは"}' | jq '.tree'
   ```
   `tree.nodes[]` + `tree.edges[]` *is* the graph. If it's wrong here, the bug is
   in `hierarchy.rs` or in the spans from step 2.

4. **Only now look at `web/`.** If nodes/edges are right but the picture is
   wrong, it's `graph-model.ts` (labels, validation throws) or `graph.ts`
   (layout).

## Worked example: `わたしは`

Step 2 output:

```json
"matches": [{ "rule_id": "hanabira-n5-040", "rule_name": "Noun は～",
              "hint": "Formation: Noun + は",
              "token_start": 1, "token_end": 1 }]
```

The rule is named **Noun は** and matches **only は**. That single line explains
the whole symptom.

Why: `grammar/hanabira/n5.toml:1017` defines the variant as

```toml
[[patterns.variants.left_context]]
one_of = [{ pos1 = "名詞" }, { pos1 = "代名詞" }, { pos1 = "形状詞" }]

[[patterns.variants.core]]
surface = "は"
```

The noun is `left_context`, and by design context never widens the span:

- `src/matcher.rs:29` — `/// Inclusive core span. Context tokens never extend this range.`
- `src/matcher.rs:220-221` — `token_start: start, token_end: core.end - 1`, where
  `start` is the *core* start. `match_left_context` (`src/matcher.rs:238`) returns
  only `end`; the context's own start position is discarded and never recorded.

Then `hierarchy.rs` does the only thing it can with an uncovered token:

- `src/hierarchy.rs:24-34` — any gap before a primary match becomes a **sibling**
  `Segment` node under the sentence root.

Result: root → [`segment-0-0` (わたし), `hanabira-n5-040` (は)]. Two nodes, side
by side.

### The shape you want is already implemented

`add_span` (`src/hierarchy.rs:62`) makes one parent node for a span and hangs
every token in that span under it as a child. So if the match claimed `0..1`
instead of `1..1`, you'd get exactly one node `わたしは` with children `わたし`
and `は`, for free. **No change to `hierarchy.rs`'s node-building or to any web
code is needed** — only the span.

## Three places you could fix it, and what each costs

Ranked by what I'd pick.

1. **Matcher, opt-in per variant (recommended).** Add a rule/variant field like
   `absorb_left_context = true`; when set, widen `token_start` back to where the
   left context began. Needs `match_left_context` to return its start (it
   currently returns only `end`), then use it at `src/matcher.rs:220`.
   - Central: fixes every host-attaching particle at once (see
     `docs/GRAPH_ISSUE_BANK.md` entry 2, same root cause with `も`).
   - **Must default to off.** `src/matcher.rs:589`
     (`sentence_final_mon_context_does_not_expand_core_span`) is a test that
     *asserts* the current narrow-span invariant. Widening unconditionally breaks
     it, and breaks ranking: span length feeds candidate comparison at
     `src/matcher.rs:441`, so silently longer spans change which match wins.

2. **Rule TOML.** Move the `left_context` steps into `core` for this rule.
   - Zero engine risk, ~2 minutes for one rule.
   - Doesn't scale (hundreds of rules), and it changes `core_specificity`, which
     feeds ranking — a bare-particle rule that suddenly has a noun in its core
     may start beating rules it used to lose to.

3. **Hierarchy.** In `build_tree`, when a gap segment sits immediately before a
   grammar node, nest the gap's tokens under the grammar node instead of adding a
   root sibling.
   - Purely presentational, no ranking impact.
   - The JSON still reports the wrong span, so anything else reading
     `primary_matches` (regression fixtures, future features) stays wrong. Fixes
     the picture, not the analysis.

## Transferable rules of thumb

- **A wrong node boundary is a wrong `token_start`/`token_end`.** Check spans
  before reading any tree-building code.
- **`left_context` / `right_context` are conditions, not coverage.** A rule can
  "know about" a token and still not claim it. Rule names like "Noun は" describe
  the *formation*, not the span.
- **The graph is depth-2 today**: root → (grammar | segment) → tokens. Grammar
  nodes never nest inside each other. Any request for real nesting (clauses) is a
  `hierarchy.rs` change, not a rendering one.
- **Gap segments are the tell.** Seeing a lone content word in its own node next
  to a particle node almost always means a particle rule matched bare and
  stranded its host.
- **Log, don't chase.** Per `docs/GRAPH_ISSUE_BANK.md`, add the sentence + a
  root-cause tag there and fix in bulk. This case is `[fragment]`.
