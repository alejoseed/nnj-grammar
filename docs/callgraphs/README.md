# Call graphs

Function-level call graphs for both binaries, hand-traced from the source
(2026-08-17).

**Start with `callgraph.html`** — open it in any browser (no server needed).
It's an interactive, collapsible call tree: start at `main()`, click a row to
see what it calls, in execution order. Tabs switch between the CLI and the
server; the search box filters and auto-expands to matches. This is much
easier to follow than the flat graphs.

The `.svg`/`.dot` files below are the same data as one flat picture — useful
for an at-a-glance overview of module boundaries, less so for tracing a path.

- `cli.dot` / `cli.svg` — the `nnj-grammar` CLI, from `src/main.rs`.
  Every `--output` format's path is drawn: blue edges are the analyzer
  path (`json`/`tree`), red edges are the legacy per-format paths
  (`table`, `raw`, `tokens`, `bunsetsu`, `bunsetsu-trace`, `graph`, `dot`).
- `server.dot` / `server.svg` — the `nnj-grammar-server` binary, from
  `src/bin/server.rs`. Startup is numbered 1–5; green edges are the
  per-request path; dotted edges are route/middleware registrations.

## How to read them

- Each box is one function; the label shows its arguments and return type.
- Boxes are clustered by source file.
- Edge labels say what data flows across the call, or the condition under
  which the branch is taken.
- Numbered, thick edges out of `Analyzer::analyze` are the pipeline order:
  tokenize → chunk → match → rank → gloss → build tree.
- Dashed self-loops mark recursion (`match_steps`, `print_tree_node`).
- Grey dashed boxes are external crates (lindera, jmdict, axum, tokio) —
  included only where they sit on the execution path.

The analysis pipeline (everything downstream of `Analyzer::analyze`) is
identical in both graphs: the CLI and the server call the exact same code.

## Regenerating

The graphs are hand-maintained; after changing call structure, edit the
`.dot` file and re-render:

```sh
# with graphviz installed
dot -Tsvg cli.dot -o cli.svg && dot -Tsvg server.dot -o server.svg

# without graphviz (WASM build via npx)
npx -y @hpcc-js/wasm-graphviz-cli cli.dot > cli.svg
npx -y @hpcc-js/wasm-graphviz-cli server.dot > server.svg
```

Not covered: `#[cfg(test)]` code, and the third binary `chunker-eval`
(`src/bin/chunker_eval.rs`), which only exercises `tokenizer` + `chunker`.
