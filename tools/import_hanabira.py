#!/usr/bin/env python3
"""Compile Hanabira's grammar catalog into explicit nnj-grammar variants.

Formation strings are treated as source notation, not as Japanese grammar.
Catalog literals become core predicates and data-defined structural hosts become
context predicates or bounded interior captures.

Usage:
    cargo build
    python3 tools/import_hanabira.py \
        /path/to/hanabira.org-japanese-content/grammar_json \
        grammar/hanabira
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SOURCE_NAME = "Hanabira Japanese Content"
SOURCE_URL = "https://github.com/tristcoil/hanabira.org-japanese-content"
JAPANESE = re.compile(r"[\u3041-\u3096\u30a1-\u30fa\u30fc\u3400-\u9fff\u3005\u3006\u30f6]+")
LITERAL = re.compile(rf"{JAPANESE.pattern}|[0-9]+|[、。！？]")
LEVEL = re.compile(r"_N([1-5])_")
ROMANIZATION = re.compile(r"\s*\([^()]*\)\s*$")
PARENTHETICAL = re.compile(r"（[^（）]*）|\([^()]*\)")
HARD_ALTERNATIVE = re.compile(r"\s*(?:\n+|[;；])\s*")
INLINE_ALTERNATIVE = re.compile(
    rf"(?P<base>{JAPANESE.pattern})\s*[（(]\s*(?:or|/)\s*(?P<alternative>{JAPANESE.pattern})\s*[）)]",
    re.IGNORECASE,
)
FURIGANA = re.compile(
    r"(?P<base>[\u3400-\u9fff\u3005\u3006]+)"
    r"[（(](?P<reading>[\u3041-\u3096\u30a1-\u30fa\u30fc]+)[）)]"
    r"(?P<suffix>[\u3041-\u3096\u30fc]*)"
)
OPTIONAL_GROUP = re.compile(
    rf"[（(]\s*(?P<leading_plus>[＋+]\s*)?(?P<choices>{JAPANESE.pattern}(?:\s*(?:/|\bor\b)\s*{JAPANESE.pattern})*)\s*[）)]",
    re.IGNORECASE,
)
SCAN = re.compile(rf"(?P<optional>__OPTIONAL_(?P<optional_value>{JAPANESE.pattern})__)|(?P<literal>{LITERAL.pattern})")
HOST_FIELDS = {"surface", "pos1", "pos2", "conj_form", "base_form"}


def toml_string(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")
    return f'"{escaped}"'


def grammar_title(title: str) -> str:
    """Remove Hanabira's final parenthesized reading/romanization."""
    return ROMANIZATION.sub("", title).strip()


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


@dataclass(frozen=True)
class HostDefinition:
    name: str
    aliases: tuple[str, ...]
    predicates: tuple[dict[str, str], ...]
    wildcard: dict[str, int] | None


@dataclass(frozen=True)
class HostMatch:
    definition: HostDefinition
    start: int
    end: int


class HostCatalog:
    def __init__(self, path: Path):
        payload = json.loads(path.read_text(encoding="utf-8"))
        if payload.get("schema_version") != 1 or not isinstance(payload.get("hosts"), list):
            raise ValueError(f"{path}: unsupported host catalog schema")

        definitions = []
        seen_aliases: dict[str, str] = {}
        for raw in payload["hosts"]:
            name = raw.get("name")
            aliases = raw.get("aliases")
            predicates = raw.get("predicates", [])
            wildcard = raw.get("wildcard")
            if not isinstance(name, str) or not name or not isinstance(aliases, list) or not aliases:
                raise ValueError(f"{path}: every host needs a name and aliases")
            if bool(predicates) == bool(wildcard):
                raise ValueError(f"{path}: host {name!r} needs predicates or a wildcard")
            for predicate in predicates:
                unknown = set(predicate) - HOST_FIELDS
                if unknown or not predicate:
                    raise ValueError(f"{path}: invalid predicate for host {name!r}: {predicate}")
            if wildcard and (
                set(wildcard) != {"min", "max"}
                or not 0 <= wildcard["min"] <= wildcard["max"]
            ):
                raise ValueError(f"{path}: invalid wildcard for host {name!r}")
            definition = HostDefinition(
                name,
                tuple(aliases),
                tuple(dict(predicate) for predicate in predicates),
                dict(wildcard) if wildcard else None,
            )
            definitions.append(definition)
            for alias in aliases:
                folded = alias.casefold()
                if folded in seen_aliases:
                    raise ValueError(
                        f"{path}: alias {alias!r} belongs to both {seen_aliases[folded]!r} and {name!r}"
                    )
                seen_aliases[folded] = name

        self.definitions = tuple(definitions)
        self.aliases = sorted(
            (
                (alias, definition)
                for definition in self.definitions
                for alias in definition.aliases
            ),
            key=lambda item: len(item[0]),
            reverse=True,
        )

    def find(self, text: str) -> list[HostMatch]:
        candidates = []
        for alias, definition in self.aliases:
            for match in re.finditer(re.escape(alias), text, re.IGNORECASE):
                before = text[match.start() - 1] if match.start() else ""
                after = text[match.end()] if match.end() < len(text) else ""
                if (before.isascii() and before.isalpha()) or (after.isascii() and after.isalpha()):
                    continue
                candidates.append(HostMatch(definition, match.start(), match.end()))

        accepted: list[HostMatch] = []
        for candidate in sorted(candidates, key=lambda item: (item.start, -(item.end - item.start))):
            if any(candidate.start < item.end and item.start < candidate.end for item in accepted):
                continue
            accepted.append(candidate)
        return sorted(accepted, key=lambda item: item.start)

    def is_host_expression(self, text: str) -> bool:
        matches = self.find(text)
        if not matches:
            return False
        remaining = list(text)
        for match in matches:
            remaining[match.start : match.end] = " " * (match.end - match.start)
        residue = "".join(remaining)
        return not JAPANESE.search(residue)


def normalize_formation(value: str) -> str:
    value = value.replace("＋", "+").replace("／", "/")
    value = re.sub(r"^\s*\d+[.)]\s*", "", value)
    value = INLINE_ALTERNATIVE.sub(
        lambda match: f"{match.group('base')}/{match.group('alternative')}", value
    )
    value = FURIGANA.sub(
        lambda match: (
            f"{match.group('base')}{match.group('suffix')} / "
            f"{match.group('reading')}{match.group('suffix')} "
        ),
        value,
    )

    def preserve_optional(match: re.Match[str]) -> str:
        prefix = value[: match.start()].rstrip()
        is_optional = bool(match.group("leading_plus")) or prefix.endswith("+")
        if not is_optional and match.start() > 0:
            is_optional = bool(JAPANESE.fullmatch(value[match.start() - 1]))
        if not is_optional:
            return " "
        choices = re.split(r"\s*(?:/|\bor\b)\s*", match.group("choices"), flags=re.IGNORECASE)
        return " / ".join(f" __OPTIONAL_{choice}__ " for choice in choices)

    value = OPTIONAL_GROUP.sub(preserve_optional, value)
    value = PARENTHETICAL.sub(
        lambda match: " ",
        value,
    )
    value = re.sub(r"\s+\bor\b\s+", " / ", value, flags=re.IGNORECASE)
    return re.sub(r"\s+", " ", value).strip()


def expand_slashes(branch: str, hosts: HostCatalog) -> list[str]:
    """Expand common A/B formation notation without concatenating choices."""
    protected = []
    last = 0
    for match in re.finditer(r"/", branch):
        left_start = max(branch.rfind(char, 0, match.start()) for char in "+,;；/") + 1
        right_ends = [index for char in "+,;；/" if (index := branch.find(char, match.end())) >= 0]
        right_end = min(right_ends, default=len(branch))
        left = branch[left_start : match.start()]
        right = branch[match.end() : right_end]
        protected.append(branch[last : match.start()])
        ascii_union = bool(
            re.search(r"[A-Za-z][A-Za-z0-9_-]*['\"]?\s*$", left)
            and re.match(r"\s*['\"]?[A-Za-z][A-Za-z0-9_-]*", right)
        )
        protected.append(
            "__HOST_UNION__"
            if ascii_union or (hosts.is_host_expression(left) and hosts.is_host_expression(right))
            else "/"
        )
        last = match.end()
    protected.append(branch[last:])
    branch = "".join(protected)

    restore = lambda value: value.replace("__HOST_UNION__", "/")
    parts = [restore(part.strip()) for part in re.split(r"\s*/\s*", branch) if part.strip()]
    if len(parts) < 2:
        return parts

    with_plus = [index for index, part in enumerate(parts) if "+" in part]
    if len(with_plus) > 1:
        if len(parts) == 2:
            left_prefix, left_choice = parts[0].rsplit("+", 1)
            right_choice, right_suffix = parts[1].split("+", 1)
            if JAPANESE.search(left_choice) and JAPANESE.search(right_choice):
                return [
                    f"{left_prefix} + {left_choice} + {right_suffix}",
                    f"{left_prefix} + {right_choice} + {right_suffix}",
                ]
        return parts
    if not with_plus:
        return parts

    index = with_plus[0]
    if index == len(parts) - 1:
        head, suffix = parts[-1].split("+", 1)
        return [f"{part} + {suffix}" for part in [*parts[:-1], head.strip()]]
    if index == 0:
        prefix, tail = parts[0].rsplit("+", 1)
        return [f"{prefix} + {part}" for part in [tail.strip(), *parts[1:]]]
    return parts


def expand_local_literal_choices(value: str) -> list[str]:
    pattern = re.compile(
        rf"(?<![A-Za-z0-9_-])(?P<left>{JAPANESE.pattern})\s*/\s*"
        rf"(?P<right>{JAPANESE.pattern})(?![A-Za-z0-9_-])"
    )
    match = pattern.search(value)
    if not match:
        return [value]
    prefix = value[: match.start()]
    suffix = value[match.end() :]
    expanded = []
    for choice in (match.group("left"), match.group("right")):
        expanded.extend(expand_local_literal_choices(prefix + choice + suffix))
    return expanded


def major_alternatives(value: str) -> list[str]:
    branches = []
    for hard in HARD_ALTERNATIVE.split(value):
        current = ""
        comma_parts = re.split(r"([,，])", hard)
        for index in range(0, len(comma_parts), 2):
            part = comma_parts[index]
            if not current:
                current = part
                continue
            right = part
            if "+" in current and "+" in re.sub(r"^\s*or\s+", "", right, flags=re.IGNORECASE):
                branches.append(current)
                current = re.sub(r"^\s*or\s+", "", right, flags=re.IGNORECASE)
            else:
                current += "," + right
        if current:
            branches.append(current)
    return branches


def formation_branches(formation: str, hosts: HostCatalog) -> list[str]:
    normalized = normalize_formation(formation)
    branches: list[str] = []
    for major in major_alternatives(normalized):
        for expanded in expand_local_literal_choices(major):
            branches.extend(expand_slashes(expanded, hosts))
    return list(dict.fromkeys(branch.strip(" .") for branch in branches if branch.strip(" .")))[:24]


@dataclass(frozen=True)
class Node:
    kind: str
    value: str
    start: int
    end: int
    optional: bool = False
    host: HostDefinition | None = None
    alternatives: tuple[HostDefinition, ...] = ()


def scan_topology(branch: str, hosts: HostCatalog) -> list[Node]:
    nodes: list[Node] = []
    for component_match in re.finditer(r"[^+]+", branch):
        component = component_match.group()
        host_matches = hosts.find(component)
        covered = [(match.start, match.end) for match in host_matches]
        host_groups: list[list[HostMatch]] = []
        for host_match in host_matches:
            if host_groups and not JAPANESE.search(
                component[host_groups[-1][-1].end : host_match.start]
            ):
                host_groups[-1].append(host_match)
            else:
                host_groups.append([host_match])
        for group in host_groups:
            definitions = tuple(
                {
                    match.definition.name: match.definition
                    for match in group
                }.values()
            )
            concrete = tuple(definition for definition in definitions if definition.predicates)
            if concrete:
                definitions = concrete
            start = min(match.start for match in group)
            end = max(match.end for match in group)
            nodes.append(
                Node(
                    "host",
                    component[start:end].strip(),
                    component_match.start() + start,
                    component_match.start() + end,
                    host=definitions[0],
                    alternatives=definitions,
                )
            )

        literal_count = 0
        for match in SCAN.finditer(component):
            if any(match.start() < end and start < match.end() for start, end in covered):
                continue
            before = component[match.start() - 1] if match.start() else ""
            after = component[match.end()] if match.end() < len(component) else ""
            if match.group("optional") is None and (
                re.match(r"[A-Za-z0-9_-]", before) or re.match(r"[A-Za-z0-9_-]", after)
            ):
                continue
            value = match.group("optional_value") if match.group("optional") else match.group()
            nodes.append(
                Node(
                    "literal",
                    value,
                    component_match.start() + match.start(),
                    component_match.start() + match.end(),
                    match.group("optional") is not None,
                )
            )
            literal_count += 1

        if not host_matches and not literal_count and re.search(r"[A-Za-z0-9～〜~…]", component):
            raise ValueError(
                f"unknown structural host {component.strip()!r} in formation branch {branch!r}"
            )
    nodes.sort(key=lambda node: (node.start, node.kind != "host"))
    return nodes


def title_branches(title: str, hosts: HostCatalog) -> list[str]:
    return formation_branches(grammar_title(title), hosts)


@dataclass
class Variant:
    id: str
    left_context: list[dict[str, Any]]
    core: list[dict[str, Any]]
    right_context: list[dict[str, Any]]


FAMILIES_PATH = (
    Path(__file__).resolve().parent.parent / "grammar" / "compiler" / "families.json"
)


class FamilyCatalog:
    """Closed-class auxiliary family registry (grammar/compiler/families.json).

    Widens a conjugating auxiliary token into the `one_of` set of its family's
    default-register members (standard), so a rule authored from one realization
    matches the whole family — e.g. a negation authored as ない also matches the
    polite ん (lemma ず) and cross-POS 無い. The token's own lemma is always
    included so the source form still matches itself even when its register is
    off by default. Membership is proven complete and fail-closed by
    tools/test_families.py against grammar/compiler/aux-inventory.json.
    """

    def __init__(self, path: Path = FAMILIES_PATH):
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        self.default_registers = set(data["default_widen_registers"])
        self.family_of: dict[tuple[str, str], str] = {}
        self.default_members: dict[str, list[dict[str, str]]] = {}
        for family in data["families"]:
            name = family["name"]
            self.default_members[name] = [
                {"pos1": m["pos1"], "base_form": m["base_form"]}
                for m in family["members"]
                if m["register"] in self.default_registers
            ]
            for member in family["members"]:
                self.family_of[(member["pos1"], member["base_form"])] = name

    def widen(self, pos1: str, base_form: str) -> list[dict[str, str]] | None:
        """`one_of` alternatives for a closed-class auxiliary token, else None."""
        name = self.family_of.get((pos1, base_form))
        if name is None:
            return None
        members = [dict(member) for member in self.default_members[name]]
        own = {"pos1": pos1, "base_form": base_form}
        if own not in members:
            members.append(own)
        return members


class Compiler:
    def __init__(
        self,
        binary: Path,
        wildcard_max: int,
        hosts: HostCatalog,
        families: "FamilyCatalog | None" = None,
    ):
        self.binary = binary
        self.wildcard_max = wildcard_max
        self.hosts = hosts
        self.families = families or FamilyCatalog()
        self.token_cache: dict[str, list[dict[str, Any]]] = {}

    def preload(self, fragments: set[str]) -> None:
        """Tokenize every unique source literal in one process."""
        ordered = sorted(fragment for fragment in fragments if fragment)
        combined = "\n".join(ordered)
        result = subprocess.run(
            [str(self.binary), "--output", "tokens", combined],
            check=True,
            capture_output=True,
            text=True,
        )
        tokens = json.loads(result.stdout)

        byte_start = 0
        for fragment in ordered:
            byte_end = byte_start + len(fragment.encode("utf-8"))
            self.token_cache[fragment] = [
                token
                for token in tokens
                if token["byte_start"] >= byte_start and token["byte_end"] <= byte_end
            ]
            byte_start = byte_end + 1

    def literal_steps(self, literal, optional=False):
        steps = []
        for token in self.token_cache.get(literal, []):
            pos1 = token["pos1"]
            base_form = token["base_form"]
            widened = self.families.widen(pos1, base_form)
            if widened is not None:
                # Closed-class auxiliary: match its whole family (negation ->
                # ない/ず/無い ...) via the proven registry, not a frozen surface
                # or a hand-guessed list.
                step: dict[str, Any] = {"one_of": widened}
            elif pos1 in ("動詞", "形容詞") and base_form and base_form != "*":
                # Open-class conjugating word: match by LEMMA so every
                # conjugation (いか/いき/いけ...) matches, not just the source form.
                step = {"pos1": pos1, "base_form": base_form}
            else:
                # Fixed particle / noun / marker: surface is the precise,
                # correct representation — freezing it is right.
                step = {"surface": token["surface"]}
            if optional:
                step["optional"] = True
            steps.append(step)
        return steps

    def host_step(self, node: Node, interior: bool) -> dict[str, Any]:
        if interior:
            return {"wildcard": {"min": 1, "max": self.wildcard_max}}
        definitions = node.alternatives or ((node.host,) if node.host else ())
        if any(definition.wildcard for definition in definitions):
            bounds = next(definition.wildcard for definition in definitions if definition.wildcard)
            return {
                "wildcard": {
                    "min": bounds["min"],
                    "max": min(bounds["max"], self.wildcard_max),
                }
            }
        predicates = []
        for definition in definitions:
            predicates.extend(definition.predicates)
        predicates = list({json.dumps(item, ensure_ascii=False, sort_keys=True): item for item in predicates}.values())
        return dict(predicates[0]) if len(predicates) == 1 else {"one_of": predicates}

    def compile_branch(self, branch: str, variant_id: str) -> Variant | None:
        nodes = scan_topology(branch, self.hosts)
        literal_indexes = [index for index, node in enumerate(nodes) if node.kind == "literal"]
        if not literal_indexes:
            return None
        first_literal = literal_indexes[0]
        last_literal = literal_indexes[-1]
        left_context = [self.host_step(node, False) for node in nodes[:first_literal] if node.kind == "host"]
        right_context = [self.host_step(node, False) for node in nodes[last_literal + 1 :] if node.kind == "host"]
        core: list[dict[str, Any]] = []
        capture_index = 1

        for node in nodes[first_literal : last_literal + 1]:
            if node.kind == "host":
                step = self.host_step(node, True)
                step["capture"] = f"slot_{capture_index}"
                core.append(step)
                capture_index += 1
            else:
                core.extend(self.literal_steps(node.value, node.optional))

        if not core:
            return None
        return Variant(variant_id, left_context, core, right_context)

    def compile_entry(self, entry: dict[str, Any]) -> list[Variant]:
        variants: list[Variant] = []
        sources = [
            ("formation", formation_branches(entry.get("formation", ""), self.hosts)),
        ]
        for source, branches in sources:
            for index, branch in enumerate(branches, start=1):
                variant = self.compile_branch(branch, f"{source}-{index:02d}")
                if variant:
                    variants.append(variant)
        if not variants:
            for index, branch in enumerate(title_branches(entry["title"], self.hosts), start=1):
                variant = self.compile_branch(branch, f"title-{index:02d}")
                if variant:
                    variants.append(variant)
        return deduplicate_variants(variants)


def deduplicate_variants(variants: list[Variant]) -> list[Variant]:
    unique: list[Variant] = []
    signatures: set[str] = set()
    for variant in variants:
        signature = json.dumps(
            [variant.left_context, variant.core, variant.right_context],
            ensure_ascii=False,
            sort_keys=True,
        )
        if signature in signatures:
            continue
        signatures.add(signature)
        unique.append(variant)
    return unique


def render_step(lines: list[str], heading: str, step: dict[str, Any]) -> None:
    lines.append(heading)
    for field in ("surface", "pos1", "pos2", "conj_form", "base_form"):
        if field in step:
            lines.append(f"{field:<12} = {toml_string(step[field])}")
    if "wildcard" in step:
        wildcard = step["wildcard"]
        lines.append(f"wildcard     = {{ min = {wildcard['min']}, max = {wildcard['max']} }}")
    if "one_of" in step:
        alternatives = []
        for predicate in step["one_of"]:
            fields = ", ".join(f"{key} = {toml_string(value)}" for key, value in predicate.items())
            alternatives.append(f"{{ {fields} }}")
        lines.append(f"one_of      = [{', '.join(alternatives)}]")
    if step.get("optional"):
        lines.append("optional     = true")
    if step.get("capture"):
        lines.append(f"capture      = {toml_string(step['capture'])}")
    lines.append("")


def render_file(
    source_path: Path,
    source_hash: str,
    level: str,
    entries: list[dict[str, Any]],
    compiler: Compiler,
) -> tuple[str, int]:
    lines = [
        f"# Generated from {SOURCE_NAME}",
        f"# Source: {SOURCE_URL}",
        "# The source repository requires attribution with a link to hanabira.org.",
        "# Regenerate with tools/import_hanabira.py; do not edit this file manually.",
        f"# Source file: {source_path.name}",
        f"# Source SHA-256: {source_hash}",
        "",
    ]
    for index, entry in enumerate(entries, start=1):
        pattern_id = f"hanabira-{level.lower()}-{index:03d}"
        variants = compiler.compile_entry(entry)
        if not variants:
            raise ValueError(f"{pattern_id}: no catalog-supplied literal anchor")

        lines.extend(
            [
                "[[patterns]]",
                f"id         = {toml_string(pattern_id)}",
                f"name       = {toml_string(grammar_title(entry['title']))}",
                f"jlpt       = {toml_string(level)}",
                f"meaning_en = {toml_string(entry.get('short_explanation', ''))}",
                f"hint       = {toml_string('Formation: ' + entry.get('formation', ''))}",
                f"sense_id   = {toml_string(pattern_id)}",
                "",
            ]
        )

        for variant in variants:
            lines.append("[[patterns.variants]]")
            lines.append(f"id              = {toml_string(variant.id)}")
            lines.append("priority        = 0")
            lines.append(f"sense_id        = {toml_string(pattern_id)}")
            lines.append("")
            for step in variant.left_context:
                render_step(lines, "[[patterns.variants.left_context]]", step)
            for step in variant.core:
                render_step(lines, "[[patterns.variants.core]]", step)
            for step in variant.right_context:
                render_step(lines, "[[patterns.variants.right_context]]", step)

    return "\n".join(lines), len(entries)


def build_regression(
    source_records: list[tuple[Path, str, str, list[dict[str, Any]]]],
) -> dict[str, Any]:
    examples = []
    files = []
    aggregate = hashlib.sha256()
    for source_path, source_hash, level, entries in source_records:
        aggregate.update(source_path.name.encode())
        aggregate.update(source_hash.encode())
        files.append(
            {
                "name": source_path.name,
                "sha256": source_hash,
                "jlpt": level,
                "entries": len(entries),
                "examples": sum(len(entry.get("examples", [])) for entry in entries),
            }
        )
        for source_index, entry in enumerate(entries, start=1):
            rule_id = f"hanabira-{level.lower()}-{source_index:03d}"
            for example_index, example in enumerate(entry.get("examples", []), start=1):
                examples.append(
                    {
                        "owning_rule_id": rule_id,
                        "jlpt": level,
                        "source_file": source_path.name,
                        "source_index": source_index,
                        "example_index": example_index,
                        "title": entry["title"],
                        "formation": entry.get("formation", ""),
                        "jp": example.get("jp", ""),
                        "romaji": example.get("romaji", ""),
                        "en": example.get("en", ""),
                    }
                )
    return {
        "schema_version": 1,
        "source": {
            "name": SOURCE_NAME,
            "url": SOURCE_URL,
            "attribution": "Source repository attribution; see hanabira.org.",
            "aggregate_sha256": aggregate.hexdigest(),
            "files": files,
        },
        "generated_by": "tools/import_hanabira.py",
        "logical_rules": sum(len(record[3]) for record in source_records),
        "examples": examples,
        "diagnostics": [],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="Hanabira grammar_json directory")
    parser.add_argument("output", type=Path, help="output directory for generated TOML and regression JSON")
    parser.add_argument(
        "--binary",
        type=Path,
        default=Path("target/debug/nnj-grammar"),
        help="compiled nnj-grammar binary used for one-batch UniDic tokenization",
    )
    parser.add_argument(
        "--hosts",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "grammar" / "compiler" / "hosts.json",
        help="JSON source-host aliases and UniDic predicates",
    )
    parser.add_argument(
        "--wildcard-max",
        type=int,
        default=24,
        help="maximum token count for an interior formation host",
    )
    args = parser.parse_args()

    if not args.source.is_dir():
        parser.error(f"source directory does not exist: {args.source}")
    if not args.binary.is_file():
        parser.error(f"binary does not exist: {args.binary}; run cargo build first")
    if args.wildcard_max < 1:
        parser.error("--wildcard-max must be at least 1")
    if not args.hosts.is_file():
        parser.error(f"host catalog does not exist: {args.hosts}")

    hosts = HostCatalog(args.hosts)

    source_files = sorted(args.source.glob("grammar_ja_N*_*.json"))
    if not source_files:
        parser.error(f"no Hanabira grammar files found in {args.source}")

    source_records: list[tuple[Path, str, str, list[dict[str, Any]]]] = []
    fragments: set[str] = set()
    seen_levels = set()
    for source_path in source_files:
        match = LEVEL.search(source_path.name)
        if not match:
            continue
        raw = source_path.read_bytes()
        entries = json.loads(raw)
        level = f"N{match.group(1)}"
        if level in seen_levels:
            parser.error(
                f"multiple source shards for {level} are not supported; merge them before import"
            )
        seen_levels.add(level)
        source_records.append((source_path, sha256(raw), level, entries))
        for entry in entries:
            branches = [
                *formation_branches(entry.get("formation", ""), hosts),
                *title_branches(entry["title"], hosts),
            ]
            for branch in branches:
                fragments.update(node.value for node in scan_topology(branch, hosts) if node.kind == "literal")

    compiler = Compiler(args.binary, args.wildcard_max, hosts)
    compiler.preload(fragments)
    total = 0
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix=f".{output.name}-", dir=output.parent))
    backup = output.with_name(f".{output.name}-previous")
    try:
        for source_path, source_hash, level, entries in source_records:
            rendered, count = render_file(source_path, source_hash, level, entries, compiler)
            output_path = stage / f"{level.lower()}.toml"
            output_path.write_text(rendered + "\n", encoding="utf-8")
            print(f"{level}: {count} patterns -> {output / output_path.name}")
            total += count

        regression = build_regression(source_records)
        regression_path = stage / "regression.json"
        regression_path.write_text(
            json.dumps(regression, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

        if backup.exists():
            shutil.rmtree(backup)
        if output.exists():
            os.replace(output, backup)
        os.replace(stage, output)
        if backup.exists():
            shutil.rmtree(backup)
    except Exception:
        if stage.exists():
            shutil.rmtree(stage)
        if backup.exists() and not output.exists():
            os.replace(backup, output)
        raise

    print(f"Regression corpus: {len(regression['examples'])} examples -> {output / 'regression.json'}")
    print(f"Compiled {total} logical patterns from {SOURCE_NAME}")


if __name__ == "__main__":
    main()
