use serde::{Deserialize, Serialize};

/// Catalog metadata assigned by the loader rather than grammar TOML.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CatalogSource {
    pub id: String,
    pub label: String,
}

impl CatalogSource {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// A grammar sense. Legacy/generated rules use `steps`; hand-authored rules may
/// use explicit `variants` when the same sense has more than one realization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatternRule {
    /// Stable rule identifier, e.g. "shika-nai".
    pub id: String,
    /// Human-readable name shown in graph output.
    pub name: String,
    /// JLPT level: "N5", "N4", ..., "N1".
    pub jlpt: String,
    #[serde(default)]
    pub meaning_en: String,
    #[serde(default)]
    pub hint: Option<String>,

    /// Legacy implicit variant. This keeps generated `[[patterns.steps]]`
    /// files source-compatible.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Explicit alternate realizations of this grammar sense.
    #[serde(default)]
    pub variants: Vec<PatternVariant>,

    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub sense_id: Option<String>,
    #[serde(default)]
    pub ambiguity_group: Option<String>,
    #[serde(default)]
    pub fallback: bool,
    #[serde(skip)]
    pub source: CatalogSource,
}

/// One deterministic realization of a rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatternVariant {
    /// Stable within the containing rule.
    pub id: String,
    /// Tokens included in the annotation span.
    #[serde(default, alias = "steps", alias = "core_steps")]
    pub core: Vec<Step>,
    /// Adjacent tokens required before the core, but excluded from its span.
    #[serde(default, alias = "left")]
    pub left_context: Vec<Step>,
    /// Adjacent tokens required after the core, but excluded from its span.
    #[serde(default, alias = "right")]
    pub right_context: Vec<Step>,
    /// Assert a clause/sentence boundary immediately before the core.
    #[serde(default, alias = "start_boundary")]
    pub left_boundary: Option<Boundary>,
    /// Assert a clause/sentence boundary immediately after the core.
    #[serde(default, alias = "end_boundary")]
    pub right_boundary: Option<Boundary>,

    /// Variant values override their rule-level counterparts when present.
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub sense_id: Option<String>,
    #[serde(default)]
    pub ambiguity_group: Option<String>,
    #[serde(default)]
    pub fallback: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Boundary {
    Clause,
    Sentence,
}

/// One sequence step. It is either a bounded wildcard or a token predicate.
/// Token predicates may be optional and may provide `one_of` alternatives.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Step {
    pub surface: Option<String>,
    pub pos1: Option<String>,
    pub pos2: Option<String>,
    pub conj_form: Option<String>,
    pub base_form: Option<String>,
    /// When set, consumes `min..=max` arbitrary tokens without crossing a
    /// clause boundary. All token predicate fields are ignored.
    pub wildcard: Option<WildcardStep>,
    #[serde(default)]
    pub optional: bool,
    /// Alternatives are ORed, then ANDed with fields directly on this step.
    #[serde(default)]
    pub one_of: Vec<TokenAlternative>,
    /// Store the consumed token range under this name in `PatternMatch`.
    #[serde(default)]
    pub capture: Option<String>,
}

impl Step {
    /// Returns true when the token satisfies the direct predicate and at least
    /// one `one_of` alternative (if alternatives were supplied).
    pub fn matches(&self, token: &crate::tokenizer::Token) -> bool {
        self.direct_matches(token)
            && (self.one_of.is_empty()
                || self
                    .one_of
                    .iter()
                    .any(|alternative| alternative.matches(token)))
    }

    fn direct_matches(&self, token: &crate::tokenizer::Token) -> bool {
        matches_field(&self.surface, &token.surface)
            && matches_field(&self.pos1, &token.pos1)
            && matches_field(&self.pos2, &token.pos2)
            && matches_field(&self.conj_form, &token.conj_form)
            && matches_field(&self.base_form, &token.base_form)
    }

    pub(crate) fn specificity(&self) -> usize {
        let direct = [
            &self.surface,
            &self.pos1,
            &self.pos2,
            &self.conj_form,
            &self.base_form,
        ]
        .into_iter()
        .filter(|field| field.is_some())
        .count();
        let alternative = self
            .one_of
            .iter()
            .map(TokenAlternative::specificity)
            .max()
            .unwrap_or(0);
        direct + alternative
    }
}

/// A `one_of` member can be a surface string or a full token predicate.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TokenAlternative {
    Surface(String),
    Token(TokenPredicate),
}

impl TokenAlternative {
    fn matches(&self, token: &crate::tokenizer::Token) -> bool {
        match self {
            Self::Surface(surface) => token.surface == *surface,
            Self::Token(predicate) => predicate.matches(token),
        }
    }

    fn specificity(&self) -> usize {
        match self {
            Self::Surface(_) => 1,
            Self::Token(predicate) => predicate.specificity(),
        }
    }
}

/// Token fields available inside a `one_of` table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenPredicate {
    pub surface: Option<String>,
    pub pos1: Option<String>,
    pub pos2: Option<String>,
    pub conj_form: Option<String>,
    pub base_form: Option<String>,
}

impl TokenPredicate {
    fn matches(&self, token: &crate::tokenizer::Token) -> bool {
        matches_field(&self.surface, &token.surface)
            && matches_field(&self.pos1, &token.pos1)
            && matches_field(&self.pos2, &token.pos2)
            && matches_field(&self.conj_form, &token.conj_form)
            && matches_field(&self.base_form, &token.base_form)
    }

    fn specificity(&self) -> usize {
        [
            &self.surface,
            &self.pos1,
            &self.pos2,
            &self.conj_form,
            &self.base_form,
        ]
        .into_iter()
        .filter(|field| field.is_some())
        .count()
    }
}

fn matches_field(expected: &Option<String>, actual: &str) -> bool {
    expected.as_deref().is_none_or(|value| value == actual)
}

/// Bounded gap. Gaps are always clause-scoped by the matcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WildcardStep {
    pub min: usize,
    pub max: usize,
}

/// Top-level structure of a grammar TOML file.
#[derive(Debug, Deserialize)]
pub struct GrammarFile {
    pub patterns: Vec<PatternRule>,
}
