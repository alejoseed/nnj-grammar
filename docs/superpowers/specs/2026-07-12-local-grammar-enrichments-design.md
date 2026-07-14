# Local Grammar Enrichments

## Goal

Make the personal offline grammar graph useful for real reading when the saved
Bunpro index omits an attested form. The first target is recognizing `かも` in
`それは……そうかも` as the casual shortened form of `かもしれない`.

## Scope

- Add optional local enrichment data to the Bunpro compilation pipeline.
- Add `Phrase + かも` to the existing `かもしれない` grammar point.
- Remove trailing pronunciation prose from source structures so `は` compiles
  as a topic marker rather than the impossible sequence `は わ`.
- Keep basic matches such as `それ` and the standalone `か` visible.
- Keep Japanese grammar knowledge out of the Rust matcher and generic compiler
  logic.

This milestone does not add vocabulary definitions, infer unknown meanings, or
build a distributable licensed grammar pack.

## Enrichment Data

The importer accepts an optional JSON enrichment file with a versioned schema:

```json
{
  "schema": "nnj.grammar-enrichments.v1",
  "rules": [
    {
      "title": "かもしれない",
      "forms": [
        { "id": "casual-short", "text": "Phrase + かも" }
      ]
    }
  ]
}
```

The personal file lives under `grammar/local/`, remains gitignored, and is
passed to `tools/import_bunpro_local.py`. Enrichments are merged by exact title
before fragment collection and compilation. They use the same formation
compiler as source forms, so no special runtime path is introduced.

The importer rejects invalid schemas, duplicate enrichment form IDs, malformed
forms, and enrichment titles that do not identify a snapshot rule. It never
silently ignores a misspelled target.

## Structure Normalization

`structure_lines` removes a trailing comma clause beginning with
`Pronounced`, case-insensitively. This converts Bunpro text such as
`Sentence topic + は, Pronounced "わ"` to `Sentence topic + は` without
encoding the Japanese particles in importer code.

## Expected Result

For `それは......そうかも」`, the local graph contains:

- `それ`: That
- `は`: As for... (Highlights sentence topic)
- `かもしれない`: Might, Maybe, Probably, spanning `か も`
- The existing basic `か` match

`そう` remains an unannotated adverb because vocabulary glossing is outside
this grammar-only milestone.

## Testing

- A Python unit test proves pronunciation prose is removed while ordinary
  comma-containing structures are preserved.
- A Python unit test proves valid enrichment forms merge into the intended rule
  and invalid targets fail.
- The tests are observed failing before implementation and passing afterward.
- The local catalog is regenerated and the exact target sentence is inspected
  in graph and JSON output.
- `cargo test --all-targets`, `cargo check`, Clippy with warnings denied, the
  Python suite, `jj status`, and `jj diff --summary` provide final verification.
