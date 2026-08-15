# nnj-grammar

Offline Japanese grammar detection. Give it a sentence and it tells you which
grammar points are in it, and where.

<!-- TODO(you): two or three sentences in your own voice. Some prompts:
     - what made you build this instead of using Bunpro/Jisho directly
     - the reading-a-sentence-you-don't-understand problem it solves for you
     - that it came out of your own Japanese study, and how you use it
     Your `building-a-grammar-detection-pipeline` blog draft is the right tone. -->

**Live demo:** <!-- TODO(you): URL once the demo ships -->

<!-- TODO(you): a GIF of the graph output goes a long way here. Terminal
     output (`--output graph`) or the D3 web view, either works. -->

No LLM is used at runtime or when generating rules. Tokenization is UniDic via
Lindera, matching is a deterministic rule database, and the whole thing runs
with no network access.

## Stack

- Rust 2021, with [Lindera](https://github.com/lindera/lindera) 3.0.7 for
  morphological analysis
- UniDic, compiled into the binary with the `embed-unidic` feature
- `rust-embed` bakes the grammar TOML files in too, so the release binary is
  self-contained and needs no `grammar/` directory at runtime
- `petgraph` for the token and match graph, `clap` for the CLI
- `axum` + `tokio` for a loopback-only local API
- `jmdict` for dictionary lookups
- TypeScript, Vite and D3 for the web graph view
- Python for the catalog importers

## Running it locally

```sh
cargo build
cargo run -- "東京しか行かない"
```

The default output is a terminal graph: the token chain with grammar
annotations underneath.

| Flag | What it does |
| :--- | :--- |
| `--output graph` | Token chain with grammar annotations (default) |
| `--output json` | Tokens plus matches, for piping or visualization |
| `--output table` | Surface, reading, POS, conjugation form, base form |
| `--output raw` | All 29 UniDic fields, numbered, for checking what each index is |
| `--output tokens` | Token array only, skips rule loading and matching |
| `--output dot` | Graphviz DOT |
| `--file <FILE>` | Read input from a file instead of an argument |
| `--grammar-db <DIR>` | Grammar rule directory, defaults to `grammar` |

### Local API

```sh
cargo run --bin nnj-grammar-server
```

Two routes: `GET /api/health` and `POST /api/analyze`. The server refuses to
bind a non-loopback address, so it cannot accidentally be exposed to the
network.

### Web graph view

Renders a committed fixture as a D3 graph without starting the Rust server.
Needs Node 26.x:

```sh
mise exec node@26 -- npm --prefix web install
mise exec node@26 -- npm --prefix web run dev
```

Vite prints the URL. The page renders `tests/fixtures/analysis-soshite.json`.

### Tests

```sh
cargo test                                        # Rust, including the regression suite
mise exec node@26 -- npm --prefix web run test    # vitest
mise exec node@26 -- npm --prefix web run test:browser  # Playwright
```

## How the matcher works

The runtime is source-neutral. The matcher implements token predicates,
variants, bounded slots and context matching, but holds no catalog-specific
Japanese grammar rules. Source host labels like `Noun` and `Verb` map to UniDic
predicates through `grammar/compiler/hosts.json`, not application code.

That separation is the point: swapping in a different grammar catalog is a data
change, not a code change.

## Grammar catalog

The default catalog is generated from
[Hanabira Japanese Content](https://github.com/tristcoil/hanabira.org-japanese-content).
UniDic tokenizes both the catalog anchors and the input text.

Hanabira's formations are human-readable rather than an executable grammar
schema, so coverage is partial by nature. The regression suite currently matches
at least 67% of its 3,310 examples and covers at least 77% of its 828 grammar
points, with no source-specific overrides. More structured local catalogs can do
better.

Regenerate after cloning Hanabira:

```sh
cargo build
python3 tools/import_hanabira.py \
  /path/to/hanabira.org-japanese-content/grammar_json \
  grammar/hanabira
```

### Local Bunpro catalog

Bunpro has a larger catalog but does not grant redistribution rights, so
`tools/import_bunpro_local.py` reads only a user-saved index payload or
minimized snapshot and writes a gitignored local database. It never logs in,
never fetches data, and never accepts credentials. Raw payloads are reduced to
IDs, titles, levels, meanings and casual/polite structure strings. Examples,
answers, audio and writeups are discarded.

```sh
python3 tools/import_bunpro_local.py \
  grammar/local/bunpro-index.bunpro-local.json \
  grammar/local \
  --enrichments grammar/local/bunpro-enrichments.bunpro-local.json

cargo run -- --grammar-db grammar/local --output graph "東京しか行かない"
```

The optional enrichment file holds personal, local-only forms missing from the
saved catalog. Schema is documented in
`docs/superpowers/specs/2026-07-12-local-grammar-enrichments-design.md`, and it
belongs under the gitignored `grammar/local/`.

Input schema for the importer:

```json
{
  "schema": "nnj.bunpro-local.v1",
  "grammar_points": [
    {
      "source_id": 249,
      "title": "しかない",
      "level": "N3",
      "meaning_en": "have no choice but; only",
      "forms": [
        { "id": "casual", "text": "Verb + しかない" },
        { "id": "polite", "text": "Verb + しかありません" }
      ]
    }
  ]
}
```

## Layout

```text
src/
├── main.rs           # CLI entry point
├── cli.rs            # argument parsing and output formats
├── tokenizer.rs      # Lindera/UniDic wrapper
├── matcher.rs        # source-neutral rule matching
├── patterns/         # rule loading and representation
├── graph/            # graph building and output rendering
├── analysis.rs       # analysis pipeline
├── analyzer.rs
├── ranking.rs        # match ranking
├── hierarchy.rs      # grammar point hierarchy
├── dictionary.rs     # jmdict lookups
├── display.rs        # terminal rendering
├── server.rs         # axum router, loopback guard
└── bin/server.rs     # server binary

grammar/
├── compiler/         # host label to UniDic predicate mapping
├── hanabira/         # generated from Hanabira
├── n1 ... n5/        # rules by JLPT level
└── local/            # gitignored, user-supplied catalogs

tools/                # Python importers
web/                  # Vite + D3 graph view
tests/                # integration tests and fixtures
```

## Project documentation

- [`PROJECT_STATUS.md`](PROJECT_STATUS.md) - progress checklist and current next action
- [`docs/CODE_TOUR.md`](docs/CODE_TOUR.md) - architecture and a recommended reading order
- [`HANDOFF.md`](HANDOFF.md) - compact session context for continuing work

## Deploying

<!-- TODO(you): nothing is committed here yet. The plan was a static demo
     (precomputed JSON + the Vite page) on Vercel, with the live Rust API on
     Fly.io later. Fill in once one of them is up. -->

## Credit

Grammar catalog generated from
[Hanabira Japanese Content](https://github.com/tristcoil/hanabira.org-japanese-content).
Morphological analysis by [Lindera](https://github.com/lindera/lindera) with
UniDic. See [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) for attribution
and licensing.