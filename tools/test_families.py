"""Fail-closed audit of the auxiliary family classification.

Proves two directions so nothing can silently fall through the cracks:
  1. Every member of the UniDic census (aux-inventory.json) is classified into
     exactly one family. A newly-appearing auxiliary halts the suite, named.
  2. Every family member exists in the census. A phantom (e.g. mistakenly listing
     the classical-perfective ぬ under negation) halts the suite, named.

Plus targeted linguistic regressions that pin the decisions we reasoned through,
so a careless edit to families.json trips a specific, readable failure.
"""

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INVENTORY = ROOT / "grammar" / "compiler" / "aux-inventory.json"
FAMILIES = ROOT / "grammar" / "compiler" / "families.json"

VALID_REGISTERS = {"standard", "formal", "classical", "dialect"}


def load(path):
    return json.loads(path.read_text(encoding="utf-8"))


class FamilyClassificationAudit(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inventory = load(INVENTORY)
        cls.families = load(FAMILIES)
        cls.inventory_keys = {
            (m["pos1"], m["base_form"]) for m in cls.inventory["members"]
        }
        cls.family_members = []
        for family in cls.families["families"]:
            for member in family["members"]:
                cls.family_members.append((family["name"], member))

    def test_every_census_member_is_classified_exactly_once(self):
        seen = {}
        for family_name, member in self.family_members:
            key = (member["pos1"], member["base_form"])
            if key in seen:
                self.fail(
                    f"{key} classified in both '{seen[key]}' and '{family_name}' "
                    "— a member may belong to exactly one family"
                )
            seen[key] = family_name

        unclassified = self.inventory_keys - set(seen)
        self.assertEqual(
            unclassified,
            set(),
            f"census members with NO family (add them to families.json): "
            f"{sorted(unclassified)}",
        )

    def test_no_phantom_family_members(self):
        phantoms = {
            (m["pos1"], m["base_form"])
            for _, m in self.family_members
        } - self.inventory_keys
        self.assertEqual(
            phantoms,
            set(),
            f"family members NOT present in the UniDic census (typo or wrong "
            f"lemma — e.g. classical-perfective vs negation): {sorted(phantoms)}",
        )

    def test_registers_are_valid(self):
        for family_name, member in self.family_members:
            self.assertIn(
                member.get("register"),
                VALID_REGISTERS,
                f"{family_name}:{member['base_form']} has invalid register "
                f"{member.get('register')!r}",
            )

    def test_default_widen_registers_declared(self):
        self.assertEqual(self.families["default_widen_registers"], ["standard"])

    # --- targeted linguistic regressions (the decisions we reasoned through) ---

    def _family_of(self, base_form):
        for name, member in self.family_members:
            if member["base_form"] == base_form:
                return name
        return None

    def _standard_members(self, family_name):
        for family in self.families["families"]:
            if family["name"] == family_name:
                return {
                    m["base_form"]
                    for m in family["members"]
                    if m["register"] == "standard"
                }
        return set()

    def test_classical_perfective_nu_is_not_negation(self):
        # 行かぬ/行かねば lemmatize to ず; the standalone lemma ぬ is the classical
        # perfective. Misfiling it under negation was a real bug in an early sketch.
        self.assertEqual(self._family_of("ぬ"), "aspect")

    def test_polite_negative_zu_is_standard_negation(self):
        # 行きません -> ん (lemma ず). Excluding ず from standard negation would
        # break every polite negative — the わけにはいきません fix depends on it.
        self.assertEqual(self._family_of("ず"), "negation")
        self.assertIn("ず", self._standard_members("negation"))

    def test_standard_negation_set_is_exactly_the_expected_three(self):
        self.assertEqual(
            self._standard_members("negation"),
            {"ない", "ず", "無い"},
            "standard-register negation must be exactly ない/ず/無い "
            "(まい is formal, へん/なんだ dialect, じ/まじ classical)",
        )

    def test_cross_pos_negation_member_present(self):
        # ではない/なくはない use 形容詞 base 無い, not 助動詞.
        self.assertIn(("形容詞", "無い"), self.inventory_keys)
        self.assertEqual(self._family_of("無い"), "negation")

    def test_census_count_matches_declared(self):
        self.assertEqual(
            self.inventory["member_count"], len(self.inventory["members"])
        )


if __name__ == "__main__":
    unittest.main()
