import unittest
from pathlib import Path

from import_bunpro_local import merge_enrichments, structure_lines
from import_hanabira import HostCatalog, formation_branches


class StructureLinesTest(unittest.TestCase):
    def test_ignores_leading_superscript_annotation_lines(self) -> None:
        source = (
            "Verb + <strong>grammar</strong><br><br>"
            "<sup>(1)</sup> Verb[alternative]<br>"
            "Noun + <strong>grammar</strong>"
        )

        self.assertEqual(
            structure_lines(source),
            ["Verb + grammar", "Noun + grammar"],
        )

    def test_removes_pronunciation_prose_only(self) -> None:
        self.assertEqual(
            structure_lines('Sentence topic + <strong>は</strong>, Pronounced "わ"'),
            ["Sentence topic + は"],
        )
        self.assertEqual(
            structure_lines("Verb + grammar, Less common"),
            ["Verb + grammar, Less common"],
        )


class EnrichmentsTest(unittest.TestCase):
    def test_merges_enrichment_forms_by_exact_title(self) -> None:
        snapshot = {
            "grammar_points": [
                {
                    "title": "かもしれない",
                    "forms": [
                        {"id": "casual", "text": "Phrase + かもしれない"}
                    ],
                }
            ]
        }

        merge_enrichments(
            snapshot,
            {
                "schema": "nnj.grammar-enrichments.v1",
                "rules": [
                    {
                        "title": "かもしれない",
                        "forms": [
                            {"id": "casual-short", "text": "Phrase + かも"}
                        ],
                    }
                ],
            },
        )

        form = snapshot["grammar_points"][0]["forms"][-1]
        self.assertEqual(form["text"], "Phrase + かも")
        self.assertTrue(form["_enrichment"])

    def test_rejects_unknown_enrichment_title(self) -> None:
        snapshot = {"grammar_points": [{"title": "known", "forms": []}]}

        with self.assertRaisesRegex(ValueError, "unknown enrichment title"):
            merge_enrichments(
                snapshot,
                {
                    "schema": "nnj.grammar-enrichments.v1",
                    "rules": [
                        {
                            "title": "missing",
                            "forms": [{"id": "x", "text": "Phrase + x"}],
                        }
                    ],
                },
            )


class FormationNotationTest(unittest.TestCase):
    def test_expands_kanji_furigana_as_surface_alternatives(self) -> None:
        hosts = HostCatalog(
            Path(__file__).resolve().parent.parent / "grammar" / "compiler" / "hosts.json"
        )

        self.assertEqual(
            formation_branches("何（なに）より + （も）+ Phrase", hosts),
            [
                "何より + __OPTIONAL_も__ + Phrase",
                "なにより + __OPTIONAL_も__ + Phrase",
            ],
        )


if __name__ == "__main__":
    unittest.main()
