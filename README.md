# nnj-grammar

Offline Japanese grammar detection using Lindera, embedded UniDic, and a
deterministic grammar-rule database.

The runtime is source-neutral: the matcher implements token predicates,
variants, bounded slots, and context matching, but contains no catalog-specific
Japanese grammar rules. Source host labels such as `Noun` and `Verb` map to
UniDic predicates through `grammar/compiler/hosts.json`, not application code.

## Grammar Catalog

The default grammar catalog is generated from
[Hanabira Japanese Content](https://github.com/tristcoil/hanabira.org-japanese-content).
UniDic tokenizes both catalog anchors and input text; no LLM is used at runtime
or during rule generation.

Hanabira's formations are human-readable rather than an executable grammar
schema. The regression suite currently matches at least 67% of its 3,310
examples and covers at least 77% of its 828 grammar points without source-
specific overrides. More structured local catalogs can provide higher coverage.

Regenerate the catalog after cloning Hanabira:

```bash
cargo build
python3 tools/import_hanabira.py \
  /path/to/hanabira.org-japanese-content/grammar_json \
  grammar/hanabira
```

See `THIRD_PARTY_NOTICES.md` for attribution and licensing notes.

## Local Bunpro Catalog

Bunpro currently has a larger catalog, but it does not grant a catalog
redistribution license. `tools/import_bunpro_local.py` therefore reads only a
user-saved Bunpro index payload or minimized snapshot and writes a gitignored
local database. It never logs in, fetches data, or accepts credentials. Raw
payloads are reduced to IDs, titles, levels, meanings, and casual/polite
structure strings; examples, answers, audio, and writeups are discarded.

Input schema:

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

Compile and use it:

```bash
cargo build
python3 tools/import_bunpro_local.py \
  grammar/local/bunpro-index.bunpro-local.json \
  grammar/local \
  --enrichments grammar/local/bunpro-enrichments.bunpro-local.json

cargo run -- --grammar-db grammar/local --output graph "東京しか行かない"
```

The optional enrichment file contains personal, local-only forms missing from
the saved source catalog. It uses the versioned schema documented in
`docs/superpowers/specs/2026-07-12-local-grammar-enrichments-design.md` and
should remain under the gitignored `grammar/local/` directory.
