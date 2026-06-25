use serde::{Deserialize, Serialize};

/// A single grammar pattern rule loaded from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternRule {
    /// Stable identifier, e.g. "shika-nai"
    pub id: String,
    /// Human-readable name shown in graph output, e.g. "しか〜ない"
    pub name: String,
    /// JLPT level: "N5", "N4", ..., "N1"
    pub jlpt: String,
    /// English meaning shown to the learner
    pub meaning_en: String,
    /// Optional usage note (e.g. "predicate must be negative")
    pub hint: Option<String>,
    /// Ordered list of steps the matcher must satisfy in sequence
    pub steps: Vec<Step>,
}

/// One matching step in a pattern rule.
///
/// A step with `wildcard` set matches 0..=max arbitrary tokens and ignores
/// all other fields. A step without `wildcard` matches exactly one token
/// where every specified field equals the token's value (unspecified fields
/// match anything).
///
/// Field values must be UniDic strings — run `nnj-grammar --output table`
/// on a test sentence to see the exact values the tokenizer emits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Match this exact surface form (e.g. "しか", "ない")
    pub surface: Option<String>,
    /// Match POS1 / major POS (品詞): 名詞, 動詞, 助詞, 助動詞, …
    pub pos1: Option<String>,
    /// Match POS2 / subcategory (品詞細分類1): 格助詞, 副助詞, 係助詞, …
    pub pos2: Option<String>,
    /// Match conjugation form (活用形): 連用形-一般, 終止形-一般, …
    pub conj_form: Option<String>,
    /// Match base/dictionary form
    pub base_form: Option<String>,
    /// When set, this step is a wildcard that consumes 0..=max tokens
    pub wildcard: Option<WildcardStep>,
}

impl Step {
    /// Returns true if this step is a wildcard (consumes a span of tokens).
    pub fn is_wildcard(&self) -> bool {
        self.wildcard.is_some()
    }

    /// Returns true if this step matches `token` (non-wildcard steps only).
    pub fn matches(&self, token: &crate::tokenizer::Token) -> bool {
        let check = |opt: &Option<String>, val: &str| -> bool {
            opt.as_deref().map_or(true, |s| s == val)
        };

        check(&self.surface, &token.surface)
            && check(&self.pos1, &token.pos1)
            && check(&self.pos2, &token.pos2)
            && check(&self.conj_form, &token.conj_form)
            && check(&self.base_form, &token.base_form)
    }
}

/// Wildcard: matches between `min` and `max` arbitrary tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WildcardStep {
    pub min: usize,
    pub max: usize,
}

/// Top-level structure of a grammar TOML file.
/// Each file contains a `patterns` array.
#[derive(Debug, Deserialize)]
pub struct GrammarFile {
    pub patterns: Vec<PatternRule>,
}
