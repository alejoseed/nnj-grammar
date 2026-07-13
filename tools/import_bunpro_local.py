#!/usr/bin/env python3
"""Compile a user-supplied Bunpro snapshot into a local grammar database.

This tool never connects to Bunpro and never accepts credentials or cookies.
Its input is a minimized personal-use snapshot using the
`nnj.bunpro-local.v1` schema documented in README.md.
"""

import argparse
import hashlib
import html
import json
import re
from pathlib import Path
from typing import Any

from import_hanabira import (
    Compiler,
    HostCatalog,
    formation_branches,
    render_step,
    scan_topology,
    toml_string,
)


SCHEMA = "nnj.bunpro-local.v1"
ENRICHMENT_SCHEMA = "nnj.grammar-enrichments.v1"


def stable_id(value: Any) -> str:
    normalized = re.sub(r"[^a-zA-Z0-9_-]+", "-", str(value)).strip("-").lower()
    if not normalized:
        raise ValueError("each grammar point needs a non-empty source_id")
    return f"bunpro-local-{normalized}"


def normalize_level(value: Any) -> str:
    match = re.fullmatch(r"(?:JLPT)?N?([1-5])", str(value).upper())
    return f"N{match.group(1)}" if match else ""


def plain_text(value: Any) -> str:
    if not isinstance(value, str):
        return ""
    without_tags = re.sub(r"<[^>]+>", " ", value)
    return re.sub(r"\s+", " ", html.unescape(without_tags)).strip()


def structure_lines(value: Any) -> list[str]:
    if not isinstance(value, str):
        return []
    text = value.replace("\u200b", "").replace("\u3000", " ")
    text = re.sub(r"<\s*/?\s*br\s*/?\s*>", "\n", text, flags=re.IGNORECASE)
    text = "\n".join(
        raw_line
        for raw_line in text.splitlines()
        if not re.match(r"^\s*<sup\b", raw_line, flags=re.IGNORECASE)
    )
    for tag in ("del", "sup", "rt", "a"):
        text = re.sub(
            rf"<{tag}\b[^>]*>.*?</{tag}\s*>",
            "",
            text,
            flags=re.IGNORECASE | re.DOTALL,
        )
    text = html.unescape(re.sub(r"<[^>]+>", "", text))

    lines = []
    for raw_line in text.splitlines():
        line = re.sub(r"\s+", " ", raw_line).strip()
        line = re.sub(r",\s*pronounced\b.*$", "", line, flags=re.IGNORECASE).rstrip()
        if not line or re.fullmatch(r"(?:examples?|exceptions?|conjugations?|negative):?", line, re.IGNORECASE):
            continue
        if re.match(r"^\s*\((?:\*|\d+)\)", line):
            continue
        line = re.sub(
            r"^(?:past form|negative|polite|casual|formal):\s*",
            "",
            line,
            flags=re.IGNORECASE,
        )
        if "→" in line or "￫" in line:
            line = re.split(r"[→￫]", line)[-1].strip()
        if line:
            lines.append(line)
    return list(dict.fromkeys(lines))


def find_catalog_arrays(value: Any) -> list[list[dict[str, Any]]]:
    candidates = []
    if isinstance(value, list):
        records = [item for item in value if isinstance(item, dict)]
        if records and all("id" in item and "title" in item for item in records):
            candidates.append(records)
        for item in value:
            candidates.extend(find_catalog_arrays(item))
    elif isinstance(value, dict):
        for child in value.values():
            candidates.extend(find_catalog_arrays(child))
    return candidates


def normalize_raw_index(payload: Any) -> dict[str, Any]:
    candidates = find_catalog_arrays(payload)
    if not candidates:
        raise ValueError("could not locate a Bunpro grammar-point array in the snapshot")
    records = max(candidates, key=len)
    points = []
    for record in records:
        forms = []
        for form_id, field in (
            ("casual", "casual_structure"),
            ("polite", "polite_structure"),
        ):
            for line_index, text in enumerate(structure_lines(record.get(field)), start=1):
                forms.append({"id": f"{form_id}-{line_index:02d}", "text": text})
        if not forms:
            title = plain_text(record.get("title"))
            if title:
                forms.append({"id": "title", "text": title})
        if not forms:
            continue
        points.append(
            {
                "source_id": record["id"],
                "title": plain_text(record.get("title")),
                "level": record.get("level", ""),
                "meaning_en": plain_text(
                    record.get("meaning") or record.get("nuance_translation")
                ),
                "forms": forms,
            }
        )
    if not points:
        raise ValueError("Bunpro snapshot contained no tokenizable grammar points")
    return {"schema": SCHEMA, "grammar_points": points}


def load_snapshot(path: Path) -> dict[str, Any]:
    snapshot = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(snapshot, dict) or snapshot.get("schema") != SCHEMA:
        snapshot = normalize_raw_index(snapshot)
    points = snapshot.get("grammar_points")
    if not isinstance(points, list) or not points:
        raise ValueError("snapshot grammar_points must be a non-empty array")
    return snapshot


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
            if (
                not isinstance(form_id, str)
                or not form_id
                or not isinstance(text, str)
                or not text
            ):
                raise ValueError(
                    f"{title}: each enrichment form needs non-empty id and text"
                )
            if form_id in seen_form_ids:
                raise ValueError(f"{title}: duplicate enrichment form id: {form_id}")
            seen_form_ids.add(form_id)
            points[title]["forms"].append(
                {
                    "id": f"enrichment-{form_id}",
                    "text": text,
                    "_enrichment": True,
                }
            )


def collect_fragments(points: list[dict[str, Any]], hosts: HostCatalog) -> set[str]:
    fragments = set()
    for point in points:
        forms = point.get("forms")
        if not isinstance(forms, list) or not forms:
            raise ValueError(f"{point.get('source_id')!r} has no forms")
        for form in forms:
            text = form.get("text", "")
            for branch in formation_branches(text, hosts):
                try:
                    fragments.update(
                        node.value
                        for node in scan_topology(branch, hosts)
                        if node.kind == "literal"
                    )
                except ValueError:
                    continue
        for branch in formation_branches(str(point.get("title", "")), hosts):
            try:
                fragments.update(
                    node.value
                    for node in scan_topology(branch, hosts)
                    if node.kind == "literal"
                )
            except ValueError:
                continue
    return fragments


def render(snapshot: dict[str, Any], compiler: Compiler, source_hash: str) -> tuple[str, int]:
    lines = [
        "# LOCAL-ONLY grammar data compiled from a user-supplied Bunpro snapshot.",
        "# Bunpro grants no catalog redistribution license. Do not commit this file.",
        f"# Input SHA-256: {source_hash}",
        "",
    ]
    seen_ids = set()
    rejected_branches = 0

    for point in snapshot["grammar_points"]:
        rule_id = stable_id(point.get("source_id"))
        if rule_id in seen_ids:
            raise ValueError(f"duplicate source_id produces {rule_id}")
        seen_ids.add(rule_id)

        variants = []
        for form_index, form in enumerate(point["forms"], start=1):
            form_id = form.get("id") or f"form-{form_index:02d}"
            for branch_index, branch in enumerate(
                formation_branches(form.get("text", ""), compiler.hosts), start=1
            ):
                try:
                    variant = compiler.compile_branch(
                        branch,
                        f"{stable_id(form_id).removeprefix('bunpro-local-')}-{branch_index:02d}",
                    )
                except ValueError as error:
                    if form.get("_enrichment"):
                        raise ValueError(
                            f"{rule_id}: invalid enrichment form {form_id!r}: {error}"
                        ) from error
                    rejected_branches += 1
                    continue
                if variant:
                    variants.append(variant)
        if not variants:
            for branch_index, branch in enumerate(
                formation_branches(str(point.get("title", "")), compiler.hosts), start=1
            ):
                try:
                    variant = compiler.compile_branch(branch, f"title-{branch_index:02d}")
                except ValueError:
                    rejected_branches += 1
                    continue
                if variant:
                    variants.append(variant)
        if not variants:
            raise ValueError(f"{rule_id} has no tokenizable catalog form or title")
        variant_ids = [variant.id for variant in variants]
        if len(set(variant_ids)) != len(variant_ids):
            raise ValueError(f"{rule_id} has duplicate compiled variant IDs")

        lines.extend(
            [
                "[[patterns]]",
                f"id         = {toml_string(rule_id)}",
                f"name       = {toml_string(str(point.get('title', '')))}",
                f"jlpt       = {toml_string(normalize_level(point.get('level', '')))}",
                f"meaning_en = {toml_string(str(point.get('meaning_en', '')))}",
                f"sense_id   = {toml_string(rule_id)}",
                "",
            ]
        )
        for variant in variants:
            lines.extend(
                [
                    "[[patterns.variants]]",
                    f"id       = {toml_string(variant.id)}",
                    f"sense_id = {toml_string(rule_id)}",
                    "",
                ]
            )
            for step in variant.left_context:
                render_step(lines, "[[patterns.variants.left_context]]", step)
            for step in variant.core:
                render_step(lines, "[[patterns.variants.core]]", step)
            for step in variant.right_context:
                render_step(lines, "[[patterns.variants.right_context]]", step)

    return "\n".join(lines) + "\n", rejected_branches


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("snapshot", type=Path, help="minimized local Bunpro JSON snapshot")
    parser.add_argument("output", type=Path, help="output directory (should remain gitignored)")
    parser.add_argument(
        "--binary",
        type=Path,
        default=Path("target/debug/nnj-grammar"),
        help="compiled nnj-grammar binary used for tokenization",
    )
    parser.add_argument("--wildcard-max", type=int, default=24)
    parser.add_argument(
        "--hosts",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "grammar" / "compiler" / "hosts.json",
    )
    parser.add_argument(
        "--enrichments",
        type=Path,
        help="optional local JSON containing additional forms for snapshot rules",
    )
    args = parser.parse_args()

    if not args.binary.is_file():
        parser.error(f"binary does not exist: {args.binary}; run cargo build first")
    if args.wildcard_max < 1:
        parser.error("--wildcard-max must be at least 1")
    project_root = Path(__file__).resolve().parent.parent
    output = args.output.resolve()
    local_root = (project_root / "grammar" / "local").resolve()
    if output.is_relative_to(project_root) and not output.is_relative_to(local_root):
        parser.error(
            f"local Bunpro output inside this repository must be under {local_root}"
        )
    snapshot = load_snapshot(args.snapshot)
    if args.enrichments:
        merge_enrichments(snapshot, load_enrichments(args.enrichments))
    hosts = HostCatalog(args.hosts)
    compiler = Compiler(args.binary, args.wildcard_max, hosts)
    compiler.preload(collect_fragments(snapshot["grammar_points"], hosts))

    raw = args.snapshot.read_bytes()
    output.mkdir(parents=True, exist_ok=True)
    destination = output / "bunpro-local.toml"
    rendered, rejected = render(snapshot, compiler, hashlib.sha256(raw).hexdigest())
    destination.write_text(rendered, encoding="utf-8")
    print(f"Compiled {len(snapshot['grammar_points'])} local grammar points -> {destination}")
    print(f"Rejected {rejected} malformed or unsupported structure branches")


if __name__ == "__main__":
    main()
