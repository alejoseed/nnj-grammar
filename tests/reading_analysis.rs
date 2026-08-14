use std::path::PathBuf;

use nnj_grammar::analysis::ANALYSIS_SCHEMA_VERSION;
use nnj_grammar::analyzer::{Analyzer, AnalyzerConfig};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn analyzer() -> Analyzer {
    Analyzer::new(AnalyzerConfig {
        local_grammar_dir: Some(fixture_dir()),
        dictionary_path: None,
    })
    .expect("reading analyzer should initialize")
}

#[test]
fn analyzer_builds_versioned_ranked_tree_with_glosses() {
    let document = analyzer()
        .analyze("そしてなによりも")
        .expect("analysis should succeed");

    assert_eq!(document.schema_version, ANALYSIS_SCHEMA_VERSION);
    assert_eq!(document.input, "そしてなによりも");
    assert_eq!(
        document
            .tokens
            .iter()
            .map(|token| token.id.as_str())
            .collect::<Vec<_>>(),
        ["token-0", "token-1", "token-2", "token-3"]
    );
    // Glosses are always on now: content words carry JMdict meanings, function
    // words (より, も) stay empty.
    let nani = &document.tokens[1];
    assert_eq!(nani.surface, "なに");
    assert!(
        nani.glosses.iter().any(|g| g.gloss.contains("what")),
        "なに should gloss to 'what', got {:?}",
        nani.glosses
    );
    assert!(document
        .primary_matches
        .iter()
        .any(|matched| matched.rule_name == "そして、～"));
    assert!(document.primary_matches.iter().any(|matched| {
        matched.rule_name == "何より" && matched.token_start == 1 && matched.token_end == 3
    }));
    assert!(document.secondary_matches.iter().any(|secondary| {
        secondary.matched.rule_name == "誰か・どこか・誰も・どこも"
            && secondary.id.starts_with("secondary-3-3-")
    }));
    // Structural tree: [そして] [なによりも], with the matches attached to
    // the bunsetsu they cover.
    assert_eq!(document.tree.root_id, "document-0");
    assert_eq!(
        document.tree.children_of("sentence-0"),
        ["bunsetsu-0-0", "bunsetsu-0-1"]
    );
    assert_eq!(
        document
            .tree
            .node("bunsetsu-0-0")
            .expect("そして bunsetsu")
            .match_ids,
        ["match-0-0"]
    );
    assert_eq!(
        document
            .tree
            .node("bunsetsu-0-1")
            .expect("なによりも bunsetsu")
            .match_ids,
        ["match-1-3"]
    );
}

#[test]
fn analyzer_covers_negative_contrast_topic_and_shortened_kamo() {
    let negative = analyzer()
        .analyze("言わないが")
        .expect("negative sentence analysis");
    let negative_rule_ids = negative
        .primary_matches
        .iter()
        .chain(
            negative
                .secondary_matches
                .iter()
                .map(|secondary| &secondary.matched),
        )
        .flat_map(|matched| matched.provenance.iter())
        .map(|provenance| provenance.rule_id.as_str())
        .collect::<Vec<_>>();
    assert!(negative_rule_ids.contains(&"test-local-negative"));
    assert!(negative_rule_ids.contains(&"test-local-contrastive-ga"));

    let kamo = analyzer()
        .analyze("それは......そうかも」")
        .expect("shortened kamo analysis");
    // The topic-は rule absorbs its host noun, so the span covers それ + は
    // rather than the bare particle.
    assert!(kamo.primary_matches.iter().any(|matched| {
        matched.token_start == 0
            && matched.token_end == 1
            && matched.rule_name.contains('は')
            && matched.meaning_en.to_lowercase().contains("topic")
    }));
    let kamo_match = kamo
        .primary_matches
        .iter()
        .find(|matched| {
            matched
                .provenance
                .iter()
                .any(|provenance| provenance.rule_id == "test-local-kamo")
        })
        .expect("shortened kamo should be primary");
    assert_eq!((kamo_match.token_start, kamo_match.token_end), (9, 10));
    assert!(kamo.secondary_matches.iter().any(|secondary| {
        secondary
            .matched
            .provenance
            .iter()
            .any(|provenance| provenance.rule_id == "test-local-question-ka")
    }));
}

#[test]
fn analyzer_is_deterministic_and_handles_long_novel_text() {
    let analyzer = analyzer();
    let sentence = "そしてなによりも";
    let first = analyzer.analyze(sentence).expect("first analysis");
    let second = analyzer.analyze(sentence).expect("second analysis");
    assert_eq!(
        serde_json::to_string_pretty(&first).expect("serialize first"),
        serde_json::to_string_pretty(&second).expect("serialize second")
    );

    let text = "今さらながらに思うんだけどさ......相手の顔色窺って様子を見てるだけっていうのは、相手を一番困らせるんだと思う";
    let long = analyzer
        .analyze(text)
        .expect("long sentence should analyze");
    assert_eq!(long.schema_version, ANALYSIS_SCHEMA_VERSION);
    assert!(!long.tokens.is_empty());
    assert_eq!(long.tree.root_id, "document-0");
    assert_eq!(
        long.tokens
            .iter()
            .map(|token| token.surface.as_str())
            .collect::<String>(),
        text
    );
    let mut expected_byte_start = 0;
    for (position, token) in long.tokens.iter().enumerate() {
        assert_eq!(token.position, position);
        assert_eq!(token.byte_start, expected_byte_start);
        assert_eq!(
            text.get(token.byte_start..token.byte_end),
            Some(token.surface.as_str())
        );
        expected_byte_start = token.byte_end;
    }
    assert_eq!(expected_byte_start, text.len());

    let clause_boundaries = ["、", ",", "，", "；", ";", "。", ".", "！", "!", "？", "?"];
    for (rule_name, token_start, token_end) in long
        .primary_matches
        .iter()
        .map(|matched| {
            (
                matched.rule_name.as_str(),
                matched.token_start,
                matched.token_end,
            )
        })
        .chain(long.secondary_matches.iter().map(|secondary| {
            (
                secondary.matched.rule_name.as_str(),
                secondary.matched.token_start,
                secondary.matched.token_end,
            )
        }))
    {
        assert!(
            long.tokens[token_start..token_end]
                .iter()
                .all(|token| !clause_boundaries.contains(&token.surface.as_str())),
            "{rule_name} crosses a clause boundary"
        );
    }
}

#[test]
fn analyzer_json_matches_the_stable_fixture() {
    let document = analyzer()
        .analyze("そしてなによりも")
        .expect("analysis should succeed");
    let actual = format!(
        "{}\n",
        serde_json::to_string_pretty(&document).expect("serialize analysis")
    );

    assert_eq!(actual, include_str!("fixtures/analysis-soshite.json"));
}

#[test]
fn analyzer_rejects_dictionary_configuration_until_dictionary_support_exists() {
    let result = Analyzer::new(AnalyzerConfig {
        local_grammar_dir: Some(fixture_dir()),
        dictionary_path: Some(PathBuf::from("grammar/local/jmdict.sqlite3")),
    });
    let error = match result {
        Ok(_) => panic!("dictionary configuration must not be silently ignored"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("dictionary support"));
}

#[test]
fn analyzer_rejects_an_invalid_configured_local_catalog_directory() {
    for invalid_path in [
        fixture_dir().join("missing"),
        fixture_dir().join("local-reading.toml"),
    ] {
        let result = Analyzer::new(AnalyzerConfig {
            local_grammar_dir: Some(invalid_path),
            dictionary_path: None,
        });
        let error = match result {
            Ok(_) => panic!("a configured local catalog must be a directory"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("local grammar directory"));
    }
}
