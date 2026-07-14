use nnj_grammar::{matcher, patterns, tokenizer::Tokenizer};

#[test]
fn public_library_tokenizes_and_matches_embedded_rules() {
    let tokenizer = Tokenizer::new().expect("embedded UniDic should initialize");
    let tokens = tokenizer.tokenize("そして").expect("tokenization should succeed");
    let rules = patterns::load_embedded().expect("embedded Hanabira should load");
    let matches = matcher::match_all(&tokens, &rules);

    assert!(matches
        .iter()
        .any(|matched| matched.rule_name.contains("そして")));
}
