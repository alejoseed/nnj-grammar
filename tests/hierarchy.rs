use nnj_grammar::analysis::{AnalysisDocument, ANALYSIS_SCHEMA_VERSION};
use nnj_grammar::hierarchy::build_tree;
use nnj_grammar::matcher::{MatchCandidate, PatternMatch};
use nnj_grammar::patterns::CatalogSource;
use nnj_grammar::ranking::rank_candidates;
use nnj_grammar::tokenizer::Token;

fn token(position: usize, surface: &str, pos1: &str) -> Token {
    Token {
        surface: surface.to_string(),
        pos1: pos1.to_string(),
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
fn structure_is_document_sentence_bunsetsu_token_and_matches_attach() {
    // [そして] [なに・より・も] — two bunsetsu from POS alone.
    let tokens = vec![
        token(0, "そして", "接続詞"),
        token(1, "なに", "代名詞"),
        token(2, "より", "助詞"),
        token(3, "も", "助詞"),
    ];
    let ranked = rank_candidates(vec![
        candidate("broad-mo", "誰か・どこか・誰も・どこも", (3, 3)),
        candidate("nani-yori", "何より", (1, 3)),
    ]);

    let tree = build_tree(&tokens, &ranked, &[]);

    assert_eq!(tree.root_id, "document-0");
    assert_eq!(tree.children_of("document-0"), ["sentence-0"]);
    assert_eq!(
        tree.children_of("sentence-0"),
        ["bunsetsu-0-0", "bunsetsu-0-1"]
    );
    assert_eq!(tree.children_of("bunsetsu-0-0"), ["token-0"]);
    assert_eq!(
        tree.children_of("bunsetsu-0-1"),
        ["token-1", "token-2", "token-3"]
    );
    // The 何より match spans exactly bunsetsu-0-1, so it labels that node;
    // the contained secondary attaches to the same smallest cover.
    let owner = tree.node("bunsetsu-0-1").expect("bunsetsu node");
    assert_eq!(owner.match_ids, ["match-1-3"]);
    assert_eq!(owner.secondary_match_ids, ["secondary-3-3-0"]);
}

#[test]
fn punctuation_attaches_to_the_preceding_bunsetsu() {
    // [前・、] [文法] [後] — punctuation can never float as its own node.
    let tokens = vec![
        token(0, "前", "名詞"),
        token(1, "、", "補助記号"),
        token(2, "文法", "名詞"),
        token(3, "後", "名詞"),
    ];
    let ranked = rank_candidates(vec![candidate("grammar", "grammar", (2, 2))]);

    let tree = build_tree(&tokens, &ranked, &[]);

    assert_eq!(tree.children_of("bunsetsu-0-0"), ["token-0", "token-1"]);
    // 文法+後 compound as adjacent nouns; the match labels that bunsetsu.
    assert_eq!(tree.children_of("bunsetsu-0-1"), ["token-2", "token-3"]);
    assert_eq!(
        tree.node("bunsetsu-0-1").expect("bunsetsu").match_ids,
        ["match-2-2"]
    );
}

#[test]
fn dictionary_word_spans_fuse_short_units_into_one_leaf() {
    // [前・、] [文法・後] with 文法+後 known to be one dictionary word.
    let tokens = vec![
        token(0, "前", "名詞"),
        token(1, "、", "補助記号"),
        token(2, "文法", "名詞"),
        token(3, "後", "名詞"),
    ];
    let tree = build_tree(&tokens, &Default::default(), &[(2, 3)]);

    assert_eq!(tree.children_of("bunsetsu-0-1"), ["word-2-3"]);
    let word = tree.node("word-2-3").expect("word leaf");
    assert_eq!(
        (word.token_start, word.token_end),
        (Some(2), Some(3)),
        "word leaf spans its short units"
    );
    assert!(tree.children_of("word-2-3").is_empty(), "words are leaves");
}

#[test]
fn analysis_document_serializes_schema_version_and_tree_root() {
    let tree = build_tree(&[], &Default::default(), &[]);
    let document = AnalysisDocument {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        input: "".to_string(),
        tokens: Vec::new(),
        primary_matches: Vec::new(),
        secondary_matches: Vec::new(),
        tree,
    };

    let json = serde_json::to_value(document).expect("serialize analysis document");

    assert_eq!(json["schema_version"], 3);
    assert_eq!(json["tree"]["root_id"], "document-0");
    assert_eq!(json["tree"]["nodes"][0]["kind"], "document");
    assert!(json["tree"]["nodes"][0]["token_start"].is_null());
    assert!(json["tree"]["nodes"][0]["token_end"].is_null());
    assert!(json["tree"]["nodes"][0]["match_ids"].is_array());
    assert!(json["primary_matches"].is_array());
    assert!(json["secondary_matches"].is_array());
}

#[test]
fn match_crossing_bunsetsu_attaches_to_the_sentence() {
    // [前が] [文法だ] — a span crossing both bunsetsu can only be covered by
    // the sentence node.
    let tokens = vec![
        token(0, "前", "名詞"),
        token(1, "が", "助詞"),
        token(2, "文法", "名詞"),
        token(3, "だ", "助動詞"),
    ];
    let mut grammar = candidate("grammar", "grammar", (2, 3));
    grammar.priority = 10;
    let ranked = rank_candidates(vec![grammar, candidate("crossing", "crossing", (0, 2))]);

    let tree = build_tree(&tokens, &ranked, &[]);

    assert_eq!(
        tree.node("bunsetsu-0-1").expect("bunsetsu").match_ids,
        ["match-2-3"]
    );
    assert_eq!(
        tree.node("sentence-0")
            .expect("sentence")
            .secondary_match_ids,
        ["secondary-0-2-0"]
    );
}
