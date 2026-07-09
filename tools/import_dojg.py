#!/usr/bin/env python3
"""
Import grammar patterns from the aiko-tanaka/Grammar-Dictionaries DoJG source
and generate TOML stub files ready for step sequences to be added.

Usage:
    python3 tools/import_dojg.py ~/Grammar-Dictionaries/dojg grammar/imported/

Each generated file contains name, meaning, and example sentences.
The 'steps' array is intentionally empty — fill it in after running:
    ./target/release/nnj-grammar --output table "<example sentence>"
"""

import json
import os
import re
import sys
from pathlib import Path


def extract_meaning(definition_text):
    """Pull the [意味] / meaning line from the definition blob."""
    # Pattern: [意味]\n<meaning text>
    match = re.search(r'\[意味\]\s*\n(.+?)(?:\n\n|\n\[)', definition_text, re.DOTALL)
    if match:
        return match.group(1).strip()
    return ""


def extract_example(definition_text):
    """Pull the first key sentence example."""
    match = re.search(r'\(ks\)[^\n]*\n[^\n]*\n(.+?)(?:\n|$)', definition_text)
    if match:
        return match.group(0).strip().split('\n')[1].strip()
    # fallback: grab any Japanese sentence
    match = re.search(r'[　-鿿]{5,}[。．]', definition_text)
    if match:
        return match.group(0)
    return ""


def surface_to_id(surface):
    """Convert a Japanese surface form to a kebab-case ASCII id."""
    # Keep the surface as-is but make it filename-safe
    safe = re.sub(r'[^\w぀-鿿]', '-', surface)
    safe = re.sub(r'-+', '-', safe).strip('-')
    return safe or 'unknown'


def load_all_entries(dojg_dir):
    entries = []
    for path in sorted(Path(dojg_dir).glob('term_bank_*.json')):
        data = json.loads(path.read_text(encoding='utf-8'))
        entries.extend(data)
    return entries


def generate_toml(surface, reading, meaning, example):
    id_slug = surface_to_id(surface)
    reading_display = reading if reading != surface else surface

    toml_lines = [
        f'# Generated from Dictionary of Japanese Grammar',
        f'# Run: nnj-grammar --output table "{example or surface + "の文"}"',
        f'# Then fill in the [[patterns.steps]] below based on the table output.',
        f'',
        f'[[patterns]]',
        f'id         = "{id_slug}"',
        f'name       = "{surface}"',
        f'jlpt       = ""  # fill in: N5 / N4 / N3 / N2 / N1',
        f'meaning_en = "{meaning}"',
    ]

    if example:
        toml_lines.append(f'# example: {example}')

    toml_lines += [
        f'',
        f'# TODO: add steps after running --output table on an example sentence',
        f'# [[patterns.steps]]',
        f'# surface = ""',
        f'# pos1    = ""',
        f'# pos2    = ""',
        f'# conj_form = ""',
    ]

    return '\n'.join(toml_lines)


def main():
    if len(sys.argv) < 3:
        print(f'Usage: {sys.argv[0]} <dojg-dir> <output-dir>')
        sys.exit(1)

    dojg_dir = sys.argv[1]
    out_dir = Path(sys.argv[2])
    out_dir.mkdir(parents=True, exist_ok=True)

    entries = load_all_entries(dojg_dir)
    print(f'Loaded {len(entries)} entries from DoJG')

    written = 0
    skipped = 0

    for entry in entries:
        surface  = entry[0]   # Japanese surface form
        reading  = entry[1]   # Reading (often same as surface for grammar points)
        def_blob = entry[5][0] if entry[5] else ''

        meaning = extract_meaning(def_blob)
        example = extract_example(def_blob)

        if not meaning:
            skipped += 1
            continue

        # One file per pattern, named by surface
        filename = surface_to_id(surface) + '.toml'
        filepath = out_dir / filename

        filepath.write_text(
            generate_toml(surface, reading, meaning, example),
            encoding='utf-8'
        )
        written += 1

    print(f'Written: {written} TOML stubs  |  Skipped (no meaning): {skipped}')
    print(f'Output:  {out_dir}')
    print()
    print('Next step: pick a pattern, run --output table on its example sentence,')
    print('then fill in the [[patterns.steps]] in the TOML file.')


if __name__ == '__main__':
    main()
