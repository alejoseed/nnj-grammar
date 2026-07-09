#!/usr/bin/env python3
# /// script
# dependencies = [
#     "toml>=0.10.2",
# ]
# ///
"""
Assign JLPT levels to grammar patterns by cross-referencing with
Grammar-Dictionaries sources, particularly nihongo_no_sensei which
has explicit JLPT level organization.

Usage:
    uv run tools/assign_jlpt.py [--dicts-dir ~/Grammar-Dictionaries]

Reads:
    - grammar/imported/*.toml (JLPT stub files)
    - ~/Grammar-Dictionaries/nihongo_no_sensei/term_bank_*.json

Writes:
    - Updates jlpt field in TOML files
    - Moves files into grammar/n5/, n4/, n3/, n2/, n1/, or unknown/ subdirectories
"""

import json
import shutil
import sys
from pathlib import Path
from typing import Dict, Optional

try:
    import tomllib
except ImportError:
    import tomli as tomllib


def load_jlpt_data(dicts_dir: Path) -> Dict[str, str]:
    """Load JLPT level mapping from Grammar-Dictionaries sources.

    Tries nihongo_kyoushi first (which organizes by JLPT level),
    then supplements with nihongo_no_sensei for additional coverage.
    """
    jlpt_map = {}

    # Try nihongo_kyoushi first - this has the exact same patterns as imported
    kyoushi_dir = dicts_dir / "nihongo_kyoushi"
    if kyoushi_dir.exists():
        print("Loading JLPT data from nihongo_kyoushi...")
        # term_bank files are organized by JLPT level:
        # term_bank_1.json = N1, term_bank_2.json = N2, etc.
        kyoushi_levels = {1: "N1", 2: "N2", 3: "N3", 4: "N4", 5: "N5", 6: "N5"}
        for bank_num in range(1, 7):
            path = kyoushi_dir / f"term_bank_{bank_num}.json"
            if path.exists():
                jlpt = kyoushi_levels.get(bank_num)
                if jlpt:
                    try:
                        data = json.loads(path.read_text(encoding="utf-8"))
                        for entry in data:
                            surface = entry[0]
                            if surface not in jlpt_map:  # Don't override
                                jlpt_map[surface] = jlpt
                    except Exception as e:
                        print(f"  Warning: error reading {path}: {e}")

    # Supplement with nihongo_no_sensei for additional patterns
    nns_dir = dicts_dir / "nihongo_no_sensei"
    if nns_dir.exists():
        print("Supplementing with nihongo_no_sensei...")
        nns_levels = {1: "N1", 2: "N2", 3: "N3", 4: "N4", 5: "N5"}
        for bank_num in range(1, 6):
            path = nns_dir / f"term_bank_{bank_num}.json"
            if path.exists():
                jlpt = nns_levels[bank_num]
                try:
                    data = json.loads(path.read_text(encoding="utf-8"))
                    for entry in data:
                        surface = entry[0]
                        if surface not in jlpt_map:  # Don't override
                            jlpt_map[surface] = jlpt
                except Exception as e:
                    print(f"  Warning: error reading {path}: {e}")

    return jlpt_map


def find_jlpt_match(name: str, jlpt_map: Dict[str, str]) -> Optional[str]:
    """Find JLPT level for a pattern name."""
    if name in jlpt_map:
        return jlpt_map[name]
    return None


def read_toml(path: Path) -> Optional[Dict]:
    """Read a TOML file and extract pattern data."""
    try:
        content = path.read_text(encoding="utf-8")
        # Parse TOML - handle parsing errors
        try:
            data = tomllib.loads(content)
        except Exception:
            # Try to parse the file by extracting just the fields we need
            # This is a fallback for malformed TOML with embedded newlines
            pattern = {}
            for line in content.split('\n'):
                if line.startswith('id'):
                    pattern['id'] = line.split('=', 1)[1].strip().strip('"')
                elif line.startswith('name'):
                    pattern['name'] = line.split('=', 1)[1].strip().strip('"')
                elif line.startswith('jlpt'):
                    val = line.split('=', 1)[1].strip().split('#')[0].strip().strip('"')
                    pattern['jlpt'] = val
            if 'name' in pattern:
                return pattern
            raise

        if "patterns" in data and len(data["patterns"]) > 0:
            return data["patterns"][0]
    except Exception as e:
        print(f"Error reading {path}: {e}")
    return None


def write_toml(path: Path, pattern: Dict, jlpt: Optional[str] = None):
    """Write updated TOML file with JLPT level."""
    if jlpt:
        pattern["jlpt"] = jlpt
    elif "jlpt" not in pattern:
        pattern["jlpt"] = ""

    lines = [
        "# Generated from Dictionary of Japanese Grammar",
        "# Run: nnj-grammar --output table \"<example sentence>\"",
        "# Then fill in the [[patterns.steps]] below based on the table output.",
        "",
        "[[patterns]]",
    ]

    # Write fields in order
    for key in ["id", "name", "jlpt"]:
        if key in pattern:
            val = pattern[key]
            lines.append(f'{key:<12} = "{val}"')

    # Skip meaning_en as it often has problematic embedded newlines
    # Just ensure jlpt is filled in

    # Add example comment if present
    lines.append("# example: <example sentence>")

    lines += [
        "",
        "# TODO: add steps after running --output table on an example sentence",
        "# [[patterns.steps]]",
        "# surface = \"\"",
        "# pos1    = \"\"",
        "# pos2    = \"\"",
        "# conj_form = \"\"",
    ]

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main():
    # Parse arguments
    dicts_dir = Path.home() / "Grammar-Dictionaries"
    for arg in sys.argv[1:]:
        if arg.startswith("--dicts-dir"):
            dicts_dir = Path(arg.split("=", 1)[1] if "=" in arg else sys.argv[sys.argv.index(arg) + 1])

    project_dir = Path(__file__).parent.parent
    imported_dir = project_dir / "grammar" / "imported"

    if not imported_dir.exists():
        print(f"Error: {imported_dir} not found")
        sys.exit(1)

    # Load JLPT data
    print(f"Loading JLPT data from {dicts_dir}...")
    jlpt_map = load_jlpt_data(dicts_dir)
    print(f"Loaded {len(jlpt_map)} JLPT mappings\n")

    # Create output directories
    jlpt_dirs = {}
    for level in ["N1", "N2", "N3", "N4", "N5"]:
        jlpt_dirs[level] = project_dir / "grammar" / level.lower()
        jlpt_dirs[level].mkdir(parents=True, exist_ok=True)

    unknown_dir = imported_dir / "unknown"
    unknown_dir.mkdir(parents=True, exist_ok=True)

    # Process TOML files from both root and unknown subdirectory
    print(f"Processing TOML files from {imported_dir}...\n")

    # Get all TOML files
    toml_files = list(imported_dir.glob("*.toml"))
    toml_files.extend(unknown_dir.glob("*.toml"))
    toml_files = sorted(set(toml_files))  # Remove duplicates

    print(f"Found {len(toml_files)} TOML files\n")

    assigned = 0
    unassigned = 0
    stats = {"N1": 0, "N2": 0, "N3": 0, "N4": 0, "N5": 0, "unknown": 0}

    for toml_path in toml_files:
        pattern = read_toml(toml_path)
        if not pattern:
            continue

        name = pattern.get("name", "")
        jlpt = find_jlpt_match(name, jlpt_map)

        if jlpt:
            # Update file content
            write_toml(toml_path, pattern, jlpt)

            # Move to appropriate directory
            dest_path = jlpt_dirs[jlpt] / toml_path.name
            if dest_path != toml_path:
                shutil.move(str(toml_path), str(dest_path))
            stats[jlpt] += 1
            assigned += 1
            if assigned <= 20:  # Only show first 20
                print(f"✓ {name:20} -> {jlpt}")
        else:
            # Move to unknown directory
            dest_path = unknown_dir / toml_path.name
            if dest_path != toml_path:
                shutil.move(str(toml_path), str(dest_path))
            stats["unknown"] += 1
            unassigned += 1
            if unassigned <= 10:  # Only show first 10
                print(f"? {name:20} -> unknown")

    if unassigned > 10:
        print(f"... and {unassigned - 10} more unassigned")

    # Summary
    print(f"\n{'='*60}")
    print(f"SUMMARY")
    print(f"{'='*60}")
    print(f"Total files: {len(toml_files)}")
    print(f"Assigned JLPT levels: {assigned}")
    print(f"Unassigned (moved to unknown/): {unassigned}")
    print(f"\nDistribution by JLPT level:")
    for level in ["N1", "N2", "N3", "N4", "N5"]:
        count = stats[level]
        pct = (count / assigned * 100) if assigned > 0 else 0
        print(f"  {level}: {count:4d} ({pct:5.1f}%)")
    print(f"\nAll files organized in:")
    print(f"  grammar/n1/, grammar/n2/, grammar/n3/, grammar/n4/, grammar/n5/")
    print(f"  grammar/imported/unknown/ (for unmatched patterns)")


if __name__ == "__main__":
    main()
