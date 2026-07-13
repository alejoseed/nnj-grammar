# Local Grammar Enrichments Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recognize `かも` as a local casual form of `かもしれない` and restore the `は` topic-marker match in `それは......そうかも」`.

**Architecture:** Keep the Rust runtime unchanged. Extend the local Bunpro importer with generic pronunciation-note cleanup and an optional, validated enrichment JSON that contributes ordinary formation strings before compilation.

**Tech Stack:** Python 3 standard library, existing formation compiler, JSON, Rust/Cargo verification.

## Global Constraints

- Keep Japanese grammar knowledge out of Rust and generic Python control flow.
- Store the personal enrichment file under gitignored `grammar/local/`.
- Preserve basic matches, including standalone `それ` and `か`.
- Do not add vocabulary glossing for `そう`.
- Do not make network requests.
- Do not create a git commit unless the user explicitly requests one.

---

### Task 1: Normalize Pronunciation Prose

**Files:**
- Modify: `tools/import_bunpro_local.py:49-85`
- Test: `tools/test_import_bunpro_local.py`

**Interfaces:**
- Consumes: `structure_lines(value: Any) -> list[str]`
- Produces: structure lines without a trailing comma clause beginning with `Pronounced`

- [ ] **Step 1: Write the failing normalization test**

Add this method to `StructureLinesTest`:

```python
def test_removes_pronunciation_prose_only(self) -> None:
    self.assertEqual(
        structure_lines('Sentence topic + <strong>は</strong>, Pronounced "わ"'),
        ["Sentence topic + は"],
    )
    self.assertEqual(
        structure_lines("Verb + grammar, Less common"),
        ["Verb + grammar, Less common"],
    )
```

- [ ] **Step 2: Run the test and confirm the expected failure**

Run from `tools/`:

```bash
python3 -m unittest test_import_bunpro_local.py
```

Expected: the new assertion receives `Sentence topic + は, Pronounced "わ"`.

- [ ] **Step 3: Add minimal generic cleanup**

In the `structure_lines` loop, after whitespace normalization and before formation parsing, add:

```python
line = re.sub(r",\s*pronounced\b.*$", "", line, flags=re.IGNORECASE).rstrip()
```

- [ ] **Step 4: Run the Python test**

Run from `tools/`:

```bash
python3 -m unittest test_import_bunpro_local.py
```

Expected: all tests pass.

---

### Task 2: Validate and Merge Local Enrichments

**Files:**
- Modify: `tools/import_bunpro_local.py`
- Test: `tools/test_import_bunpro_local.py`

**Interfaces:**
- Produces: `load_enrichments(path: Path) -> dict[str, Any]`
- Produces: `merge_enrichments(snapshot: dict[str, Any], payload: dict[str, Any]) -> None`
- Consumes enrichment schema `nnj.grammar-enrichments.v1`

- [ ] **Step 1: Write failing merge and validation tests**

Import `merge_enrichments` and add these tests:

```python
def test_merges_enrichment_forms_by_exact_title(self) -> None:
    snapshot = {
        "grammar_points": [
            {"title": "かもしれない", "forms": [{"id": "casual", "text": "Phrase + かもしれない"}]}
        ]
    }
    merge_enrichments(
        snapshot,
        {
            "schema": "nnj.grammar-enrichments.v1",
            "rules": [
                {
                    "title": "かもしれない",
                    "forms": [{"id": "casual-short", "text": "Phrase + かも"}],
                }
            ],
        },
    )
    self.assertEqual(snapshot["grammar_points"][0]["forms"][-1]["text"], "Phrase + かも")
    self.assertTrue(snapshot["grammar_points"][0]["forms"][-1]["_enrichment"])

def test_rejects_unknown_enrichment_title(self) -> None:
    snapshot = {"grammar_points": [{"title": "known", "forms": []}]}
    with self.assertRaisesRegex(ValueError, "unknown enrichment title"):
        merge_enrichments(
            snapshot,
            {
                "schema": "nnj.grammar-enrichments.v1",
                "rules": [{"title": "missing", "forms": [{"id": "x", "text": "Phrase + x"}]}],
            },
        )
```

- [ ] **Step 2: Run the tests and confirm import failure**

Run from `tools/`:

```bash
python3 -m unittest test_import_bunpro_local.py
```

Expected: import fails because `merge_enrichments` does not exist.

- [ ] **Step 3: Implement schema validation and merging**

Add `ENRICHMENT_SCHEMA = "nnj.grammar-enrichments.v1"`, then implement:

```python
def load_enrichments(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("enrichment file must contain an object")
    return payload


def merge_enrichments(snapshot: dict[str, Any], payload: dict[str, Any]) -> None:
    if payload.get("schema") != ENRICHMENT_SCHEMA:
        raise ValueError(f"enrichment schema must be {ENRICHMENT_SCHEMA}")
    rules = payload.get("rules")
    if not isinstance(rules, list):
        raise ValueError("enrichment rules must be an array")

    points = {point.get("title"): point for point in snapshot["grammar_points"]}
    seen_titles = set()
    for rule in rules:
        if not isinstance(rule, dict) or not isinstance(rule.get("title"), str):
            raise ValueError("each enrichment rule needs a title")
        title = rule["title"]
        if title in seen_titles:
            raise ValueError(f"duplicate enrichment title: {title}")
        seen_titles.add(title)
        if title not in points:
            raise ValueError(f"unknown enrichment title: {title}")

        forms = rule.get("forms")
        if not isinstance(forms, list) or not forms:
            raise ValueError(f"{title}: enrichment forms must be a non-empty array")
        seen_form_ids = set()
        for form in forms:
            if not isinstance(form, dict):
                raise ValueError(f"{title}: each enrichment form must be an object")
            form_id = form.get("id")
            text = form.get("text")
            if not isinstance(form_id, str) or not form_id or not isinstance(text, str) or not text:
                raise ValueError(f"{title}: each enrichment form needs non-empty id and text")
            if form_id in seen_form_ids:
                raise ValueError(f"{title}: duplicate enrichment form id: {form_id}")
            seen_form_ids.add(form_id)
            points[title]["forms"].append(
                {"id": f"enrichment-{form_id}", "text": text, "_enrichment": True}
            )
```

- [ ] **Step 4: Make malformed enrichment branches fatal**

In `render`, retain the existing rejection count for source branches, but re-raise compilation errors when `form.get("_enrichment")` is true:

```python
except ValueError as error:
    if form.get("_enrichment"):
        raise ValueError(
            f"{rule_id}: invalid enrichment form {form_id!r}: {error}"
        ) from error
    rejected_branches += 1
    continue
```

- [ ] **Step 5: Run the Python tests**

Run from `tools/`:

```bash
python3 -m unittest test_import_bunpro_local.py
```

Expected: all tests pass.

---

### Task 3: Wire the Personal Enrichment into Compilation

**Files:**
- Modify: `tools/import_bunpro_local.py:256-296`
- Modify: `README.md:34-72`
- Create locally: `grammar/local/bunpro-enrichments.bunpro-local.json`

**Interfaces:**
- Adds CLI option: `--enrichments PATH`
- Uses `load_enrichments` and `merge_enrichments` before fragment preloading

- [ ] **Step 1: Add the optional CLI argument**

Add:

```python
parser.add_argument(
    "--enrichments",
    type=Path,
    help="optional local JSON containing additional forms for snapshot rules",
)
```

After `snapshot = load_snapshot(args.snapshot)`, add:

```python
if args.enrichments:
    merge_enrichments(snapshot, load_enrichments(args.enrichments))
```

- [ ] **Step 2: Create the gitignored personal enrichment file**

Create `grammar/local/bunpro-enrichments.bunpro-local.json`:

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

- [ ] **Step 3: Document local compilation**

Update the README command to include:

```bash
python3 tools/import_bunpro_local.py \
  grammar/local/bunpro-index.bunpro-local.json \
  grammar/local \
  --enrichments grammar/local/bunpro-enrichments.bunpro-local.json
```

Explain that enrichment files are personal, local-only additions using the schema in the design specification.

- [ ] **Step 4: Regenerate the catalog**

Run:

```bash
python3 tools/import_bunpro_local.py \
  grammar/local/bunpro-index.bunpro-local.json \
  grammar/local \
  --enrichments grammar/local/bunpro-enrichments.bunpro-local.json
```

Expected: 979 grammar points compile and the generated TOML contains an `enrichment-casual-short-01` variant under `かもしれない`.

---

### Task 4: Verify Reading Output and Regressions

**Files:**
- Verify: `grammar/local/bunpro-local.toml`
- Verify: all source and test changes

**Interfaces:**
- Consumes the regenerated local catalog through `--grammar-db grammar/local`
- Produces graph and JSON evidence for the exact target sentence

- [ ] **Step 1: Run the exact sentence as a graph**

```bash
cargo run --quiet -- --grammar-db grammar/local --output graph 'それは......そうかも」'
```

Expected: matches include `それ`, `は`, and `かもしれない`; the latter spans `か` and `も`.

- [ ] **Step 2: Inspect exact JSON spans**

```bash
cargo run --quiet -- --grammar-db grammar/local --output json 'それは......そうかも」'
```

Expected: `bunpro-local-113` has `token_start: 9` and `token_end: 10`; `bunpro-local-3` matches token 1.

- [ ] **Step 3: Run all verification commands**

```bash
python3 -m unittest discover -s tools -p 'test_*.py'
cargo test --all-targets
cargo check
cargo clippy --all-targets -- -D warnings
git diff --check
```

Expected: every command exits successfully, Python reports all tests passing, and Rust reports 13 tests passing.
