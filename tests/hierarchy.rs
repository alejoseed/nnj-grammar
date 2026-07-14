use nnj_grammar::analysis::{AnalysisDocument, ANALYSIS_SCHEMA_VERSION};
use nnj_grammar::hierarchy::build_tree;
use nnj_grammar::matcher::{MatchCandidate, PatternMatch};
use nnj_grammar::patterns::CatalogSource;
use nnj_grammar::ranking::rank_candidates;
use nnj_grammar::tokenizer::Token;

fn token(position: usize, surface: &str) -> Token {
    Token {
        surface: surface.to_string(),
        pos1: String::new(),
        pos2: String::new(),
        pos3: String::new(),
        pos4: String::new(),
        conj_type: String::new(),
        conj_form: String::new(),
        base_form: surface.to_string(),
        reading: surface.to_string(),
        byte_start: position,
        byte_end: position + surface.len(),
        position,
    }
}

fn candidate(rule_id: &str, name: &str, span: (usize, usize)) -> MatchCandidate {
    MatchCandidate {
        matched: PatternMatch {
            rule_id: rule_id.to_string(),
            variant_id: "default".to_string(),
            rule_name: name.to_string(),
            jlpt: "N5".to_string(),
            meaning_en: name.to_string(),
            hint: None,
            sense_id: Some(rule_id.to_string()),
            ambiguity_group: None,
            source: CatalogSource::new("test", "Test"),
            captures: Vec::new(),
            token_start: span.0,
            token_end: span.1,
        },
        fallback: false,
        priority: 0,
        core_specificity: 1,
        context_specificity: 0,
        wildcard_steps: 0,
        optional_steps: 0,
        discovery_order: 0,
    }
}

#[test]
fn grammar_nodes_preserve_source_order_and_own_their_tokens() {
    let tokens = vec![
        token(0, "そして"),
        token(1, "なに"),
        token(2, "より"),
        token(3, "も"),
    ];
    let ranked = rank_candidates(vec![
        candidate("broad-mo", "誰か・どこか・誰も・どこも", (3, 3)),
        candidate("nani-yori", "何より", (1, 3)),
    ]);

    let tree = build_tree(&tokens, &ranked);

    assert_eq!(tree.root_id, "sentence-0");
    assert_eq!(tree.children_of("sentence-0"), ["segment-0-0", "match-1-3"]);
    assert_eq!(
        tree.children_of("match-1-3"),
        ["token-1", "token-2", "token-3"]
    );
    assert_eq!(
        tree.node("match-1-3")
            .expect("grammar node")
            .secondary_match_ids,
        ["secondary-3-3-0"]
    );
    assert_eq!(
        tree.edges
            .iter()
            .map(|edge| (edge.parent_id.as_str(), edge.child_id.as_str(), edge.order))
            .collect::<Vec<_>>(),
        [
            ("sentence-0", "segment-0-0", 0),
            ("segment-0-0", "token-0", 0),
            ("sentence-0", "match-1-3", 1),
            ("match-1-3", "token-1", 0),
            ("match-1-3", "token-2", 1),
            ("match-1-3", "token-3", 2),
        ]
    );
}

#[test]
fn adjacent_uncovered_tokens_form_ordered_segments_with_punctuation() {
    let tokens = vec![
        token(0, "前"),
        token(1, "、"),
        token(2, "文法"),
        token(3, "後"),
    ];
    let ranked = rank_candidates(vec![candidate("grammar", "grammar", (2, 2))]);

    let tree = build_tree(&tokens, &ranked);

    assert_eq!(
        tree.children_of("sentence-0"),
        ["segment-0-1", "match-2-2", "segment-3-3"]
    );
    assert_eq!(tree.children_of("segment-0-1"), ["token-0", "token-1"]);
    assert_eq!(tree.children_of("segment-3-3"), ["token-3"]);
}

#[test]
fn analysis_document_serializes_schema_version_and_tree_root() {
    let tree = build_tree(&[], &Default::default());
    let document = AnalysisDocument {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        input: "".to_string(),
        tokens: Vec::new(),
        primary_matches: Vec::new(),
        secondary_matches: Vec::new(),
        tree,
    };

    let json = serde_json::to_value(document).expect("serialize analysis document");

    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["tree"]["root_id"], "sentence-0");
    assert_eq!(json["tree"]["nodes"][0]["kind"], "sentence");
    assert!(json["tree"]["nodes"][0]["token_start"].is_null());
    assert!(json["tree"]["nodes"][0]["token_end"].is_null());
    assert!(json["primary_matches"].is_array());
    assert!(json["secondary_matches"].is_array());
}

#[test]
fn crossing_secondary_match_attaches_to_sentence_root() {
    let tokens = vec![
        token(0, "前"),
        token(1, "文法"),
        token(2, "点"),
        token(3, "後"),
    ];
    let mut grammar = candidate("grammar", "grammar", (1, 2));
    grammar.priority = 10;
    let ranked = rank_candidates(vec![grammar, candidate("crossing", "crossing", (0, 1))]);

    let tree = build_tree(&tokens, &ranked);

    assert_eq!(
        tree.node("sentence-0")
            .expect("sentence root")
            .secondary_match_ids,
        ["secondary-0-1-0"]
    );
}
