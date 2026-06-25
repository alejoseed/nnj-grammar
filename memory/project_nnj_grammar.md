---
name: project-nnj-grammar
description: Context for the nnj-grammar project — Japanese grammar pattern graph builder in Rust
metadata:
  type: project
---

nnj-grammar is a Rust CLI tool for Japanese grammar pattern detection aimed at the developer's personal language learning. It tokenizes Japanese text using lindera (embedded IPADIC), matches POS-tag sequences against TOML-defined grammar pattern rules, and outputs a JSON graph annotating identified grammar constructions.

**Why:** Developer notices grammar constructions (e.g., しか requiring a negative predicate) even when knowing all vocabulary in a sentence. Wanted a fast, offline, deterministic NLP approach — no LLMs.

**Key decisions made:**
- Language: Rust (portability, single binary, speed)
- Tokenizer: lindera 3.0.7 with embed-ipadic (zero system deps)
- Grammar DB source: Hanabira.org CC-licensed content for N5 (legal redistribution); Bunpro personal-use only
- Output: JSON graph (custom serializer, not petgraph native serde) + optional DOT
- Pattern format: TOML step-based POS sequence matcher with wildcard gaps

**Plan:** docs/plans/2026-05-30-001-feat-japanese-grammar-graph-builder-plan.md

**Known open issues from doc review:**
- embed-ipadic downloads dictionary at BUILD time (not runtime), contradicting "zero dictionary download" claim — needs LINDERA_DICTIONARIES_PATH caching for CI
- Grammar DB TOML files load from CWD at runtime, breaking portability for PATH-installed binary — should embed with include_str! or rust-embed
- Wildcard step algorithm needs min-to-max ascending scan, NOT greedy-max-first (greedy will fail span constructions like しか〜ない)
- R7 (100ms) needs a Criterion benchmark
- N5-only grammar DB may be too thin for intermediate learners (plan scopes to personal use)

**How to apply:** Check the plan and open issues before suggesting implementation approach. The CWD grammar issue and wildcard algorithm are correctness blockers that must be fixed before U4 implementation.
