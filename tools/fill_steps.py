#!/usr/bin/env python3
# /// script
# dependencies = ["toml>=0.10.2"]
# ///
"""
Automatically fill [[patterns.steps]] in grammar stub files.

For each pattern stub that has no steps, this script:
  1. Tokenizes the pattern's id (or a provided sample phrase) via the binary
  2. Converts each token to a matching step using UniDic POS heuristics
  3. Writes the updated TOML back to disk

Run from the project root:
    uv run tools/fill_steps.py [--dry-run] [--dirs n3 n2 n1 imported/unknown]

The binary must be built first:
    cargo build --release
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

import toml

BINARY = "./target/release/nnj-grammar"

# For patterns whose id alone doesn't tokenize into the right morphemes,
# provide a minimal context phrase, how many leading tokens to skip, and
# how many tokens to take (None = take all remaining).
# Format: id -> (phrase_to_tokenize, leading_tokens_to_skip, max_tokens_to_take)
SAMPLE_OVERRIDES: dict[str, tuple[str, int, int | None]] = {
    # Suffix patterns — need a preceding verb stem
    "がたい":       ("書きがたい",       1, None),  # 書き + がたい
    "がち":         ("休みがち",         1, None),  # 休み + がち (exclude だ)
    "かねない":     ("言いかねない",     1, None),
    "かねる":       ("言いかねる",       1, None),
    "きる":         ("食べきる",         1, None),
    "きれない":     ("食べきれない",     1, None),
    "っぽい":       ("子供っぽい",       1, None),
    "すぎる":       ("食べすぎる",       1, None),
    "がする":       ("においがする",     1, None),

    # Patterns with noun/adjective base — use with a dummy noun
    "らしい":       ("学生らしい",       1, None),
    "らしく":       ("学生らしく",       1, None),
    "ぽい":         ("子供ぽい",         1, None),
    "さ":           ("高さ",             1, None),

    # から as particle — needs noun context (alone it tokenizes as 接続詞)
    "からなる":     ("山からなる",       1, None),
    "から言って":   ("事実から言って",   1, None),
    "からこそ":     ("努力からこそ",     1, None),
    "からといって": ("忙しいからといって", 1, None),
    "からには":     ("約束からには",     1, None),
    "からして":     ("態度からして",     1, None),
    "からすると":   ("様子からすると",   1, None),
    "からすれば":   ("様子からすれば",   1, None),

    # ず/ぬ — tokenize as 記号 alone; need verb context
    "ずとも":       ("知らずとも",       1, None),
    "ずに":         ("知らずに",         1, None),
    "ずにはいられない": ("知らずにはいられない", 1, None),
    "ずして":       ("知らずして",       1, None),
    "ぬ":           ("知らぬ",           1, None),

    # たら — tokenizes as conjunction alone; needs verb context
    "たらどうですか": ("行ったらどうですか", 1, None),  # skip 行っ; たら is one token
    "たらよかった": ("行ったらよかった", 2, None),

    # Patterns starting with と — need noun context for correct POS
    "といえば":     ("映画といえば",     1, None),
    "というのは":   ("彼というのは",     1, None),
    "といった":     ("映画といった",     1, None),
    "とか":         ("映画とか",         1, None),
    "として":       ("学生として",       1, None),
    "としては":     ("学生としては",     1, None),
    "とは":         ("映画とは",         1, None),
    "とも":         ("映画とも",         1, None),
    "と共に":       ("彼と共に",         1, None),

    # や — listing particle; alone it tokenizes as 形状詞
    "や":           ("本や",             1, None),

    # ば patterns — need verb context
    "ばよかった":   ("行けばよかった",   1, None),
}

# Patterns that represent standalone sentence adverbs / conjunctions.
# These don't need the preceding wildcard since they appear sentence-initially
# and match_all already tries every position.
STANDALONE_ADVERBS = {
    "あまり", "いくら", "つまり", "つい", "どんなに",
    "まず", "もし", "やはり", "やっぱり", "きっと",
    "たしか", "おそらく", "どうせ", "なぜなら", "ただ",
    "なぜ", "なんと", "しかし", "でも", "だから", "それで",
    "そして", "また", "あるいは", "あるいわ", "さすが",
    "しかも", "そのうえ", "さて", "ところが", "ところで",
    "なお", "むしろ", "かえって", "せっかく", "せめて",
    "よく", "すぐ", "もちろん", "とくに", "とにかく",
    "けれども", "したがって", "そもそも", "そこで", "すると",
    "おそらく", "おそく", "ちゃんと",
}


def tokenize(phrase: str) -> list[dict]:
    result = subprocess.run(
        [BINARY, "--output", "json", phrase],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip())
    data = json.loads(result.stdout)
    return data["tokens"]


def token_to_step(token: dict) -> dict:
    """Convert a token dict to a matching step dict using UniDic POS heuristics."""
    pos1 = token["pos1"]
    pos2 = token.get("pos2", "")
    surface = token["surface"]
    base_form = token.get("base_form", "")

    step: dict = {}

    if pos1 == "助詞":
        # Particles don't conjugate — surface is sufficient and precise.
        step["surface"] = surface
        step["pos1"] = pos1
        if pos2:
            step["pos2"] = pos2

    elif pos1 == "助動詞":
        # Auxiliary verbs conjugate — match base_form so all forms fire.
        step["pos1"] = pos1
        if base_form:
            step["base_form"] = base_form

    elif pos1 == "動詞" and pos2 == "非自立可能":
        # Auxiliary-function verbs (いる, する, くる, もらう…) — base_form.
        step["pos1"] = pos1
        step["pos2"] = pos2
        if base_form:
            step["base_form"] = base_form

    elif pos1 == "動詞":
        # Content verbs inside a grammar marker (e.g. よる in によって).
        step["pos1"] = pos1
        if base_form:
            step["base_form"] = base_form

    elif pos1 == "名詞":
        # Nominals in grammar markers (こと, もの, はず, わけ…) — use surface.
        step["surface"] = surface
        if pos2 and pos2 not in ("普通名詞", "固有名詞"):
            step["pos2"] = pos2

    elif pos1 in ("副詞", "接続詞", "感動詞"):
        step["surface"] = surface
        step["pos1"] = pos1

    elif pos1 == "形容詞":
        step["pos1"] = pos1
        if base_form:
            step["base_form"] = base_form

    elif pos1 == "形状詞":
        step["pos1"] = pos1
        if base_form:
            step["base_form"] = base_form

    else:
        step["surface"] = surface
        step["pos1"] = pos1

    return step


def toml_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def write_toml(filepath: Path, patterns: list[dict]) -> None:
    """Serialize patterns back to a clean TOML file."""
    lines: list[str] = []

    for pat in patterns:
        lines.append("[[patterns]]")
        lines.append(f'id         = {toml_str(pat["id"])}')
        lines.append(f'name       = {toml_str(pat["name"])}')
        lines.append(f'jlpt       = {toml_str(pat.get("jlpt", ""))}')
        if pat.get("meaning_en"):
            lines.append(f'meaning_en = {toml_str(pat["meaning_en"])}')
        if pat.get("hint"):
            lines.append(f'hint       = {toml_str(pat["hint"])}')
        lines.append("")

        for step in pat.get("steps", []):
            lines.append("[[patterns.steps]]")
            if "wildcard" in step:
                w = step["wildcard"]
                lines.append(f'wildcard   = {{ min = {w["min"]}, max = {w["max"]} }}')
            else:
                for key in ("surface", "pos1", "pos2", "conj_form", "base_form"):
                    if key in step:
                        lines.append(f"{key:<10} = {toml_str(step[key])}")
            lines.append("")

    filepath.write_text("\n".join(lines) + "\n")


def process_file(filepath: Path, dry_run: bool) -> tuple[int, int]:
    """Return (patterns_filled, patterns_skipped)."""
    try:
        data = toml.loads(filepath.read_text())
    except Exception as e:
        print(f"  [TOML ERROR] {filepath}: {e}", file=sys.stderr)
        return 0, 0

    patterns = data.get("patterns", [])
    filled = 0
    skipped = 0
    modified = False

    for pat in patterns:
        pid = pat.get("id", "")

        if pat.get("steps"):
            skipped += 1
            continue

        # Determine what to tokenize
        if pid in SAMPLE_OVERRIDES:
            phrase, skip, take = SAMPLE_OVERRIDES[pid]
        else:
            phrase, skip, take = pid, 0, None

        try:
            tokens = tokenize(phrase)
        except Exception as e:
            print(f"  [TOKENIZE FAIL] {pid}: {e}", file=sys.stderr)
            skipped += 1
            continue

        tokens = tokens[skip:]
        if take is not None:
            tokens = tokens[:take]
        if not tokens:
            print(f"  [NO TOKENS] {pid}", file=sys.stderr)
            skipped += 1
            continue

        steps = [token_to_step(t) for t in tokens]
        pat["steps"] = steps
        filled += 1
        modified = True

        token_desc = " | ".join(
            f'{t["surface"]}({t["pos1"]}{"/" + t["pos2"] if t["pos2"] else ""})'
            for t in tokens
        )
        print(f"  {pid}: {len(steps)} steps  [{token_desc}]")

    if modified and not dry_run:
        write_toml(filepath, patterns)

    return filled, skipped


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="print changes without writing files")
    parser.add_argument("--dirs", nargs="+", default=["n3", "n2", "n1", "imported/unknown"],
                        help="grammar subdirectories to process (relative to grammar/)")
    args = parser.parse_args()

    grammar_root = Path("grammar")
    if not grammar_root.exists():
        sys.exit("Run from the project root (grammar/ not found)")

    if not Path(BINARY).exists():
        sys.exit(f"Binary not found: {BINARY}\nRun: cargo build --release")

    total_filled = total_skipped = total_files = 0

    for subdir in args.dirs:
        dirpath = grammar_root / subdir
        if not dirpath.exists():
            print(f"Skipping missing directory: {dirpath}")
            continue

        toml_files = sorted(dirpath.glob("*.toml"))
        print(f"\n=== {dirpath} ({len(toml_files)} files) ===")

        for filepath in toml_files:
            total_files += 1
            filled, skipped = process_file(filepath, args.dry_run)
            total_filled += filled
            total_skipped += skipped

    print(f"\nDone. {total_files} files processed, {total_filled} patterns filled, {total_skipped} already had steps.")
    if args.dry_run:
        print("(dry run — no files written)")


if __name__ == "__main__":
    main()
