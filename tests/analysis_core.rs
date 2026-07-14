use std::fs;

use nnj_grammar::{matcher, patterns, ranking, tokenizer::Tokenizer};
use tempfile::tempdir;

#[test]
fn combined_catalog_prioritizes_soshite_and_nani_yori() {
    let local = tempdir().expect("temporary local catalog");
    fs::write(
        local.path().join("bunpro-local.toml"),
        r#"
            [[patterns]]
            id = "bunpro-local-nani-yori"
            name = "何より"
            jlpt = "N2"
            meaning_en = "Above all else, More than anything"
            sense_id = "bunpro-local-nani-yori"

            [[patterns.variants]]
            id = "casual"
            [[patterns.variants.core]]
            surface = "なに"
            [[patterns.variants.core]]
            surface = "より"
            [[patterns.variants.core]]
            surface = "も"
            optional = true

            [[patterns]]
            id = "bunpro-local-broad-mo"
            name = "誰か・どこか・誰も・どこも"
            jlpt = "N5"
            meaning_en = "Someone, Somewhere, Not anyone, Not anywhere"
            sense_id = "bunpro-local-broad-mo"

            [[patterns.variants]]
            id = "casual"
            [[patterns.variants.left_context]]
            pos1 = "代名詞"
            [[patterns.variants.left_context]]
            pos1 = "助詞"
            [[patterns.variants.core]]
            surface = "も"
        "#,
    )
    .expect("write local grammar fixture");

    let tokenizer = Tokenizer::new().expect("embedded UniDic");
    let tokens = tokenizer
        .tokenize("そしてなによりも")
        .expect("tokenization should succeed");
    let rules = patterns::load_combined(Some(local.path())).expect("combined catalog");
    let ranked = ranking::rank_candidates(matcher::match_candidates(&tokens, &rules));

    assert_eq!(
        ranked
            .primary
            .iter()
            .map(|matched| matched.rule_name.as_str())
            .collect::<Vec<_>>(),
        ["そして、～", "何より"]
    );
    assert!(ranked.secondary.iter().any(|secondary| {
        secondary.matched.rule_name == "誰か・どこか・誰も・どこも"
            && secondary.matched.token_start == 3
            && secondary.matched.token_end == 3
    }));
}
