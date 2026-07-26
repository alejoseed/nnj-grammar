//! Sentinel tests for the UniDic lemma conventions that the auxiliary family
//! registry (grammar/compiler/families.json) depends on.
//!
//! families.json classifies auxiliaries by their (pos1, base_form) lemma. If a
//! lindera or UniDic upgrade ever changes how these surfaces lemmatize, the
//! family widening in the importer would silently stop matching. These tests are
//! the tripwire: they fail loudly and point at the exact convention that drifted.

use nnj_grammar::tokenizer::{Token, Tokenizer};

fn find<'a>(tokens: &'a [Token], surface: &str) -> &'a Token {
    tokens
        .iter()
        .find(|t| t.surface == surface)
        .unwrap_or_else(|| panic!("no token with surface {surface:?} in {tokens:?}"))
}

#[test]
fn auxiliary_lemma_conventions_match_the_family_registry() {
    let tokenizer = Tokenizer::new().expect("embedded UniDic should initialize");

    // Negation: plain ない stays 助動詞/ない.
    let nai = tokenizer.tokenize("行かない").expect("tokenize");
    let t = find(&nai, "ない");
    assert_eq!((t.pos1.as_str(), t.base_form.as_str()), ("助動詞", "ない"));

    // Polite negative ん lemmatizes to ず — this is why ず is the standard
    // negation member, not a classical-only one.
    let masen = tokenizer.tokenize("行きません").expect("tokenize");
    let n = find(&masen, "ん");
    assert_eq!(n.base_form, "ず", "polite ん must lemmatize to ず");

    // Negative ぬ ALSO lemmatizes to ず. The standalone lemma ぬ is the classical
    // PERFECTIVE (classified under 'aspect'), so negation must key on ず, not ぬ.
    let nu = tokenizer.tokenize("行かぬ").expect("tokenize");
    let neg_nu = find(&nu, "ぬ");
    assert_eq!(
        neg_nu.base_form, "ず",
        "negative ぬ must lemmatize to ず, not ぬ (ぬ is classical perfective)"
    );

    // Cross-POS negation: じゃない's ない is 形容詞/無い, not 助動詞. families.json
    // lists 無い (形容詞) as a standard negation member for exactly this.
    let janai = tokenizer.tokenize("じゃない").expect("tokenize");
    let nai_adj = find(&janai, "ない");
    assert_eq!(
        (nai_adj.pos1.as_str(), nai_adj.base_form.as_str()),
        ("形容詞", "無い"),
        "じゃない's ない must be 形容詞/無い (cross-POS negation)"
    );

    // Conjecture stem: そう is 形状詞 with the disambiguated lemma そう-様態.
    let sou = tokenizer.tokenize("高そうだ").expect("tokenize");
    let sou_tok = find(&sou, "そう");
    assert_eq!(
        (sou_tok.pos1.as_str(), sou_tok.base_form.as_str()),
        ("形状詞", "そう-様態"),
    );
}
