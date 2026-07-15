# Hanabira-Faithful Fixture Graph

## Purpose

Build the first visible `nnj-grammar` interface as a faithful reproduction of
Hanabira's grammar graph. This slice proves that the deterministic Rust
`AnalysisDocument` can drive the intended left-to-right reading visualization
before the loopback API, reading card, history, responsive focus mode, or JMdict
integration exists.

The visual references are:

- <https://hanabira.org/grammar-graph>
- <https://github.com/tristcoil/hanabira.org>
- `frontend-next/src/components-parser/ParseTree.tsx` in that repository
- `frontend-next/public/img/screenshots/hanabira_grammar_graph.png` in that
  repository

The live page currently returns a Cloudflare 403 to automated requests. The
active `ParseTree.tsx` implementation and repository screenshot are therefore
the source of truth. The Hanabira application code is MIT licensed.

## Approved Decisions

- Use a private Vite application under `web/` with TypeScript, D3 7, and
  Tailwind CSS 4 through `@tailwindcss/vite`.
- Do not add React. The graph is an imperative D3 renderer mounted by a small
  TypeScript entry point.
- Render the committed `tests/fixtures/analysis-soshite.json` document directly
  for the first slice.
- Reproduce the Hanabira graph canvas rather than inventing a new visual
  identity.
- Preserve the reference appearance and intentional interactions, but fix the
  accidental first-pan/zoom jump.
- Scope this slice to the graph canvas. Input, API calls, reading cards,
  secondary-candidate disclosure, history, graph controls, responsive focus
  mode, and static asset embedding remain later milestones.
- Keep Japanese analysis and inference in Rust. TypeScript may only validate,
  index, and present the supplied schema.

## Architecture

The first web application has these units:

### `web/src/types.ts`

Mirror every schema version 1 record consumed by the graph: analysis document,
tokens, display matches, secondary matches, tree nodes, and tree edges. Keep
tree node kinds as a discriminated union.

### `web/src/graph-model.ts`

Convert the normalized node and edge arrays into a presentation hierarchy. This
module owns structural validation, child ordering, document indexing, and label
derivation. It has no DOM or D3 rendering concerns.

Validation requires:

- `schema_version === 1`.
- Exactly one root matching `tree.root_id`.
- Unique node IDs.
- Existing parent and child references for every edge.
- Exactly one parent for every non-root node.
- No cycles or disconnected nodes.
- Edge order preserved within each parent.
- Referenced token and match IDs present in the document.

### `web/src/graph.ts`

Render one validated presentation hierarchy into a supplied HTML host. This
module owns D3 layout, SVG creation, links, nodes, labels, hover/focus
transitions, and zoom behavior. It does not parse API responses or infer label
meaning.

### `web/src/main.ts`

Load `tests/fixtures/analysis-soshite.json` through a Vite-resolved URL, validate
it, build the presentation hierarchy, and mount the graph. Render a concise
error in the graph host when loading or validation fails. Do not render a
partial hierarchy.

### Tailwind Boundary

`web/src/styles.css` contains the Tailwind import. Static page, host, and SVG
presentation use literal Tailwind utility classes so Vite can discover them.
D3 sets attributes only when values are data-driven or animated, such as node
coordinates, path data, hover radius, hover fill, and zoom transforms. Do not
create a parallel handwritten component stylesheet.

## Document-To-Graph Mapping

The Rust hierarchy remains authoritative. The TypeScript view model follows
tree IDs into document records and derives labels as follows:

| Node kind | Primary label | Secondary label |
|---|---|---|
| Sentence | Empty, matching Hanabira's unlabeled root | Empty |
| Grammar | Exact surface reconstructed from the inclusive token span | Match `meaning_en` |
| Segment | Concatenated token surfaces in source order | Empty |
| Token | Token `surface` | First English gloss, otherwise `reading` when different from surface |

Grammar nodes resolve `match_id`; token nodes resolve `token_id`. Segment and
grammar surface reconstruction uses document token order and rejects invalid
spans. Empty or missing glosses do not invent English text.

Secondary matches stay in `AnalysisDocument` but are not drawn in this
graph-only slice. The later reading card will expose them without changing the
main hierarchy.

## Visual Fidelity Contract

### Canvas

- SVG view box: `0 0 1200 800`.
- Preserve aspect ratio: `xMidYMid meet`.
- Responsive width and height within a fixed 3:2 coordinate space.
- Background: `#f1f5f9`.
- Base font: `12px sans-serif`, allowing the system CJK fallback.
- Reference margins: top 20, right 120, bottom 20, left 200.
- Inner layout size: 880 by 760.

### Layout

- Build a `d3.hierarchy` from the validated presentation model.
- Use `d3.tree().size([760, 586.67])` with D3's default separation.
- Render horizontally with screen x equal to `d.y * 0.8` and screen y equal to
  `d.x + 40`.
- Preserve child order from each edge's `order` field.

### Links

- Use `d3.linkHorizontal()` with the same coordinate mapping as nodes.
- Stroke `#94a3b8`, width 1, opacity 0.6, and no fill.
- Do not reproduce Hanabira's unreachable depth-zero link-color branch.

### Nodes

- Circle radius 6.
- Root fill `#1f77b4`.
- Every descendant fill `#4daf4a`; do not introduce POS or JLPT colors.
- Stroke `#555`, width 1.
- Assign stable DOM IDs derived from the already stable tree node IDs.
- Add accessible labels and keyboard focusability without changing the visual
  hierarchy.

### Labels

- Primary label: 12px sans-serif, `#333`.
- Internal primary: x -10, dy -1.5em, end anchored.
- Leaf primary: x 10, dy .35em, start anchored.
- Secondary label: 10px sans-serif, italic, `#555`, wrapped in parentheses.
- Internal secondary: x -10, dy -.5em, end anchored.
- Leaf secondary: x 10, dy 1.5em, start anchored.
- Do not draw either label for the sentence root.

### Hover And Focus

- Over 200ms, increase radius from 6 to 10 and change circle and label color to
  `#ff7f0e`.
- Make labels bold while emphasized.
- Reverse those properties over 200ms when hover or focus leaves.
- Use the same emphasis for keyboard focus as pointer hover.
- Do not add an in-SVG tooltip in this slice.

### Pan And Zoom

- Attach `d3.zoom()` with scale extent 0.5 through 2.
- Retain D3 defaults for wheel zoom, drag pan, and double-click zoom.
- Do not add zoom buttons or automatic fit behavior in this slice.
- Use a zoom viewport group containing a separately translated margin group.
  Zoom updates only the viewport group, preserving the initial margin and
  preventing Hanabira's first-interaction jump.

## Error Handling

Fixture fetch, JSON parsing, schema mismatch, invalid graph structure, and
missing document references produce a concise visible error in the graph host.
The renderer receives only a validated model. It must not silently drop invalid
nodes or edges.

Console output must not include passage text. This fixture is public test data,
but the same boundary will later receive private user input.

## Testing

### Vitest Model Tests

Cover:

- Schema version enforcement.
- Exactly one root.
- Stable child and source-token order.
- Missing node, token, and match references.
- Duplicate parent and duplicate node rejection.
- Cycle and disconnected-node rejection.
- Exact labels for sentence, grammar, segment, and token nodes.
- Gloss-first and non-redundant-reading fallback behavior.

### jsdom Renderer Tests

Assert:

- The 1200 by 800 view box and slate canvas.
- Expected node and link counts from the committed fixture.
- Blue root and green descendant classes or attributes.
- An unlabeled root.
- Exact Japanese surfaces and grammar meanings.
- Stable DOM IDs, accessible labels, and focusability.
- Separate viewport and margin groups.

### Playwright Browser Test

Run Chromium against the fixture page and verify:

- The responsive SVG is visible.
- Pointer hover applies orange emphasis.
- Keyboard focus applies equivalent emphasis.
- Wheel input changes scale within the allowed range.
- Drag input changes pan position.
- The first zoom or pan does not remove the initial margin.

Pixel-perfect screenshot assertions are intentionally avoided because system
CJK font rendering varies. Fable performs a visual screenshot comparison
against Hanabira's repository screenshot after browser verification passes.

## OpenCode And Fable Workflow

Work is sequential to prevent shared-worktree collisions.

1. OpenCode creates the Vite/Tailwind toolchain, schema types, model conversion,
   entry-point contract, and failing tests.
2. OpenCode stops editing and gives the user a precise Fable prompt.
3. Fable implements the visual renderer and Tailwind presentation only in
   `web/index.html`, `web/src/graph.ts`, and `web/src/styles.css`. OpenCode does
   not edit those files during Fable's pass; Fable does not edit the toolchain,
   model, entry point, or tests.
4. Fable records its implementation summary and visual review in
   `claude-output.md`.
5. OpenCode reads the handoff, reviews the actual diff against this design and
   Hanabira's source, challenges unjustified deviations, fixes integration or
   accessibility defects, and runs the complete verification suite.
6. If another visual pass is needed, OpenCode supplies a targeted prompt and
   waits before touching Fable-owned files again.

`claude-output.md` is an ephemeral collaboration mailbox. Add it to
`.gitignore`; do not include it in product commits. The existing `.superpowers/`
brainstorming artifacts remain ignored as well.

## First-Slice Completion Criteria

- Node.js 26.x is selected before installing dependencies.
- `npm --prefix web run dev` renders the committed fixture without the Rust
  server.
- The graph is recognizably faithful to Hanabira's repository screenshot and
  exact active source constants.
- The accidental first-interaction zoom jump is absent.
- Type checking, Vitest, and Playwright checks pass.
- Fable's fidelity review has no unresolved visual discrepancy within this
  slice's scope.
- Existing Rust and Python checks remain unchanged and passing.
