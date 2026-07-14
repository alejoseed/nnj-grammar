use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::patterns::{Boundary, CatalogSource, PatternRule, PatternVariant, Step};
use crate::tokenizer::Token;

/// One named token range captured while matching a rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub struct PatternCapture {
    pub name: String,
    pub token_start: usize,
    pub token_end: usize,
}

/// One grammar construction found in the token stream.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PatternMatch {
    pub rule_id: String,
    pub variant_id: String,
    pub rule_name: String,
    pub jlpt: String,
    pub meaning_en: String,
    pub hint: Option<String>,
    pub sense_id: Option<String>,
    pub ambiguity_group: Option<String>,
    #[serde(skip_serializing)]
    pub source: CatalogSource,
    pub captures: Vec<PatternCapture>,
    /// Inclusive core span. Context tokens never extend this range.
    pub token_start: usize,
    pub token_end: usize,
}

#[derive(Clone)]
struct SequenceMatch {
    end: usize,
    captures: Vec<PatternCapture>,
    specificity: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MatchCandidate {
    pub matched: PatternMatch,
    pub fallback: bool,
    pub core_specificity: usize,
    pub context_specificity: usize,
    pub priority: i32,
    pub wildcard_steps: usize,
    pub optional_steps: usize,
    #[serde(skip)]
    pub discovery_order: usize,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct CandidateKey {
    rule_id: String,
    variant_id: String,
    token_start: usize,
    token_end: usize,
    captures: Vec<PatternCapture>,
    fallback: bool,
    core_specificity: usize,
    context_specificity: usize,
    priority: i32,
    wildcard_steps: usize,
    optional_steps: usize,
}

struct EffectiveVariant<'a> {
    id: &'a str,
    core: &'a [Step],
    left_context: &'a [Step],
    right_context: &'a [Step],
    left_boundary: Option<Boundary>,
    right_boundary: Option<Boundary>,
    priority: i32,
    sense_id: Option<&'a str>,
    ambiguity_group: Option<&'a str>,
    fallback: bool,
}

/// Find all grammar construction matches, retaining unrelated overlapping and
/// nested spans. Duplicate senses and ambiguity groups are resolved only when
/// they annotate the same core span.
pub fn match_all(tokens: &[Token], rules: &[PatternRule]) -> Vec<PatternMatch> {
    let mut first_by_occurrence: HashMap<(String, String, usize), MatchCandidate> =
        HashMap::new();
    for candidate in match_candidates(tokens, rules) {
        let key = (
            candidate.matched.rule_id.clone(),
            candidate.matched.variant_id.clone(),
            candidate.matched.token_start,
        );
        let replace = first_by_occurrence
            .get(&key)
            .is_none_or(|current| candidate.discovery_order < current.discovery_order);
        if replace {
            first_by_occurrence.insert(key, candidate);
        }
    }
    let mut candidates: Vec<_> = first_by_occurrence.into_values().collect();
    candidates.sort_by_key(|candidate| candidate.discovery_order);
    resolve_candidates(candidates)
}

/// Return every distinct successful parse together with ranking evidence.
pub fn match_candidates(tokens: &[Token], rules: &[PatternRule]) -> Vec<MatchCandidate> {
    let mut candidates = Vec::new();

    for rule in rules {
        if !rule.steps.is_empty() {
            let variant = EffectiveVariant {
                id: "default",
                core: &rule.steps,
                left_context: &[],
                right_context: &[],
                left_boundary: None,
                right_boundary: None,
                priority: rule.priority,
                sense_id: rule.sense_id.as_deref(),
                ambiguity_group: rule.ambiguity_group.as_deref(),
                fallback: rule.fallback,
            };
            collect_variant_matches(tokens, rule, &variant, &mut candidates);
        }

        for explicit in &rule.variants {
            let variant = effective_variant(rule, explicit);
            collect_variant_matches(tokens, rule, &variant, &mut candidates);
        }
    }

    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate_key(candidate)));
    candidates.sort_by(candidate_identity_order);
    candidates
}

fn candidate_key(candidate: &MatchCandidate) -> CandidateKey {
    CandidateKey {
        rule_id: candidate.matched.rule_id.clone(),
        variant_id: candidate.matched.variant_id.clone(),
        token_start: candidate.matched.token_start,
        token_end: candidate.matched.token_end,
        captures: candidate.matched.captures.clone(),
        fallback: candidate.fallback,
        core_specificity: candidate.core_specificity,
        context_specificity: candidate.context_specificity,
        priority: candidate.priority,
        wildcard_steps: candidate.wildcard_steps,
        optional_steps: candidate.optional_steps,
    }
}

fn effective_variant<'a>(
    rule: &'a PatternRule,
    variant: &'a PatternVariant,
) -> EffectiveVariant<'a> {
    EffectiveVariant {
        id: &variant.id,
        core: &variant.core,
        left_context: &variant.left_context,
        right_context: &variant.right_context,
        left_boundary: variant.left_boundary,
        right_boundary: variant.right_boundary,
        priority: variant.priority.unwrap_or(rule.priority),
        sense_id: variant.sense_id.as_deref().or(rule.sense_id.as_deref()),
        ambiguity_group: variant
            .ambiguity_group
            .as_deref()
            .or(rule.ambiguity_group.as_deref()),
        fallback: variant.fallback.unwrap_or(rule.fallback),
    }
}

fn collect_variant_matches(
    tokens: &[Token],
    rule: &PatternRule,
    variant: &EffectiveVariant<'_>,
    candidates: &mut Vec<MatchCandidate>,
) {
    let steps = variant
        .left_context
        .iter()
        .chain(variant.core)
        .chain(variant.right_context);
    let wildcard_steps = steps.clone().filter(|step| step.wildcard.is_some()).count();
    let optional_steps = steps.filter(|step| step.optional).count();

    for start in 0..tokens.len() {
        if !has_boundary(tokens, start, variant.left_boundary, true) {
            continue;
        }
        let Some(left) = match_left_context(tokens, start, variant.left_context) else {
            continue;
        };

        for core in match_sequence(tokens, start, variant.core) {
            if core.end <= start || !has_boundary(tokens, core.end, variant.right_boundary, false) {
                continue;
            }
            let Some(right) = match_right_context(tokens, core.end, variant.right_context) else {
                continue;
            };

            let mut captures = left.captures.clone();
            captures.extend(core.captures);
            captures.extend(right.captures);
            candidates.push(MatchCandidate {
                matched: PatternMatch {
                    rule_id: rule.id.clone(),
                    variant_id: variant.id.to_string(),
                    rule_name: rule.name.clone(),
                    jlpt: rule.jlpt.clone(),
                    meaning_en: rule.meaning_en.clone(),
                    hint: rule.hint.clone(),
                    sense_id: variant.sense_id.map(str::to_owned),
                    ambiguity_group: variant.ambiguity_group.map(str::to_owned),
                    source: rule.source.clone(),
                    captures,
                    token_start: start,
                    token_end: core.end - 1,
                },
                fallback: variant.fallback,
                core_specificity: core.specificity,
                context_specificity: left.specificity
                    + right.specificity
                    + usize::from(variant.left_boundary.is_some())
                    + usize::from(variant.right_boundary.is_some()),
                priority: variant.priority,
                wildcard_steps,
                optional_steps,
                discovery_order: candidates.len(),
            });
        }
    }
}

fn match_left_context(
    tokens: &[Token],
    core_start: usize,
    steps: &[Step],
) -> Option<SequenceMatch> {
    if steps.is_empty() {
        return Some(SequenceMatch {
            end: core_start,
            captures: Vec::new(),
            specificity: 0,
        });
    }

    // Prefer stronger evidence, then the nearest viable start. The sequence
    // must end adjacent to core and never changes the annotation span.
    let mut best: Option<(usize, SequenceMatch)> = None;
    for context_start in 0..=core_start {
        for matched in match_sequence(tokens, context_start, steps) {
            if matched.end != core_start {
                continue;
            }
            let replace = best.as_ref().is_none_or(|(best_start, current)| {
                (matched.specificity, context_start) > (current.specificity, *best_start)
            });
            if replace {
                best = Some((context_start, matched));
            }
        }
    }
    best.map(|(_, matched)| matched)
}

fn match_right_context(tokens: &[Token], core_end: usize, steps: &[Step]) -> Option<SequenceMatch> {
    if steps.is_empty() {
        return Some(SequenceMatch {
            end: core_end,
            captures: Vec::new(),
            specificity: 0,
        });
    }
    match_sequence(tokens, core_end, steps)
        .into_iter()
        .max_by_key(|matched| (matched.specificity, matched.end))
}

/// Enumerate every successful end position in deterministic, non-greedy order.
fn match_sequence(tokens: &[Token], start: usize, steps: &[Step]) -> Vec<SequenceMatch> {
    let mut matches = Vec::new();
    let mut captures = Vec::new();
    match_steps(tokens, start, steps, 0, &mut captures, &mut matches);
    matches
}

fn match_steps(
    tokens: &[Token],
    position: usize,
    steps: &[Step],
    specificity: usize,
    captures: &mut Vec<PatternCapture>,
    matches: &mut Vec<SequenceMatch>,
) {
    let Some((step, remaining)) = steps.split_first() else {
        matches.push(SequenceMatch {
            end: position,
            captures: captures.clone(),
            specificity,
        });
        return;
    };

    if let Some(wildcard) = &step.wildcard {
        for count in wildcard.min..=wildcard.max {
            let Some(after) = position.checked_add(count) else {
                break;
            };
            if after > tokens.len() {
                break;
            }
            if tokens[position..after].iter().any(is_clause_boundary) {
                break;
            }
            let capture_len = captures.len();
            if count > 0 {
                push_capture(captures, step, position, after);
            }
            match_steps(tokens, after, remaining, specificity, captures, matches);
            captures.truncate(capture_len);
        }
        return;
    }

    if position < tokens.len() && step.matches(&tokens[position]) {
        let capture_len = captures.len();
        push_capture(captures, step, position, position + 1);
        match_steps(
            tokens,
            position + 1,
            remaining,
            specificity + step.specificity(),
            captures,
            matches,
        );
        captures.truncate(capture_len);
    }
    if step.optional {
        match_steps(tokens, position, remaining, specificity, captures, matches);
    }
}

fn push_capture(captures: &mut Vec<PatternCapture>, step: &Step, start: usize, end: usize) {
    if let Some(name) = &step.capture {
        captures.push(PatternCapture {
            name: name.clone(),
            token_start: start,
            token_end: end - 1,
        });
    }
}

fn has_boundary(tokens: &[Token], position: usize, boundary: Option<Boundary>, left: bool) -> bool {
    let Some(boundary) = boundary else {
        return true;
    };
    let neighboring = if left {
        position.checked_sub(1).and_then(|index| tokens.get(index))
    } else {
        tokens.get(position)
    };
    neighboring.is_none_or(|token| match boundary {
        Boundary::Clause => is_clause_boundary(token),
        Boundary::Sentence => is_sentence_boundary(token),
    })
}

fn is_clause_boundary(token: &Token) -> bool {
    matches!(
        token.surface.as_str(),
        "、" | "," | "，" | "；" | ";" | "。" | "." | "！" | "!" | "？" | "?"
    )
}

fn is_sentence_boundary(token: &Token) -> bool {
    matches!(token.surface.as_str(), "。" | "." | "！" | "!" | "？" | "?")
}

fn resolve_candidates(mut candidates: Vec<MatchCandidate>) -> Vec<PatternMatch> {
    candidates.sort_by(candidate_identity_order);

    let mut semantic = Vec::<MatchCandidate>::new();
    for candidate in candidates {
        if let Some(index) = semantic.iter().position(|existing| {
            same_span(existing, &candidate) && same_sense_or_rule(existing, &candidate)
        }) {
            if better_candidate(&candidate, &semantic[index]) {
                semantic[index] = candidate;
            }
        } else {
            semantic.push(candidate);
        }
    }

    let mut resolved = Vec::<MatchCandidate>::new();
    for candidate in semantic {
        if let Some(index) = resolved.iter().position(|existing| {
            same_span(existing, &candidate)
                && existing.matched.ambiguity_group.is_some()
                && existing.matched.ambiguity_group == candidate.matched.ambiguity_group
        }) {
            if better_candidate(&candidate, &resolved[index]) {
                resolved[index] = candidate;
            }
        } else {
            resolved.push(candidate);
        }
    }

    resolved.sort_by(candidate_identity_order);
    resolved
        .into_iter()
        .map(|candidate| candidate.matched)
        .collect()
}

fn same_span(left: &MatchCandidate, right: &MatchCandidate) -> bool {
    left.matched.token_start == right.matched.token_start
        && left.matched.token_end == right.matched.token_end
}

fn same_sense_or_rule(left: &MatchCandidate, right: &MatchCandidate) -> bool {
    left.matched.rule_id == right.matched.rule_id
        || (left.matched.sense_id.is_some() && left.matched.sense_id == right.matched.sense_id)
}

fn better_candidate(left: &MatchCandidate, right: &MatchCandidate) -> bool {
    match left.fallback.cmp(&right.fallback).reverse() {
        Ordering::Greater => return true,
        Ordering::Less => return false,
        Ordering::Equal => {}
    }
    match (
        left.core_specificity,
        left.context_specificity,
        left.priority,
        left.matched.token_end - left.matched.token_start,
    )
        .cmp(&(
            right.core_specificity,
            right.context_specificity,
            right.priority,
            right.matched.token_end - right.matched.token_start,
        )) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => {
            (&left.matched.rule_id, &left.matched.variant_id)
                < (&right.matched.rule_id, &right.matched.variant_id)
        }
    }
}

fn candidate_identity_order(left: &MatchCandidate, right: &MatchCandidate) -> Ordering {
    (
        left.matched.token_start,
        left.matched.token_end,
        &left.matched.rule_id,
        &left.matched.variant_id,
        left.core_specificity,
        left.context_specificity,
        left.wildcard_steps,
        left.optional_steps,
        left.discovery_order,
    )
        .cmp(&(
            right.matched.token_start,
            right.matched.token_end,
            &right.matched.rule_id,
            &right.matched.variant_id,
            right.core_specificity,
            right.context_specificity,
            right.wildcard_steps,
            right.optional_steps,
            right.discovery_order,
        ))
}

#[cfg(test)]
mod tests {
    use super::{match_all, match_candidates};
    use crate::patterns::rule::{Boundary, PatternVariant, TokenAlternative, WildcardStep};
    use crate::patterns::{PatternRule, Step};
    use crate::tokenizer::Token;

    fn token(surface: &str, pos1: &str, position: usize) -> Token {
        Token {
            surface: surface.to_string(),
            pos1: pos1.to_string(),
            pos2: String::new(),
            pos3: String::new(),
            pos4: String::new(),
            conj_type: String::new(),
            conj_form: String::new(),
            base_form: surface.to_string(),
            reading: String::new(),
            byte_start: position,
            byte_end: position + 1,
            position,
        }
    }

    fn tokens(values: &[(&str, &str)]) -> Vec<Token> {
        values
            .iter()
            .enumerate()
            .map(|(position, (surface, pos1))| token(surface, pos1, position))
            .collect()
    }

    fn surface(value: &str) -> Step {
        Step {
            surface: Some(value.to_string()),
            ..Step::default()
        }
    }

    fn pos1(value: &str) -> Step {
        Step {
            pos1: Some(value.to_string()),
            ..Step::default()
        }
    }

    fn wildcard(min: usize, max: usize) -> Step {
        Step {
            wildcard: Some(WildcardStep { min, max }),
            ..Step::default()
        }
    }

    fn rule(id: &str, steps: Vec<Step>) -> PatternRule {
        PatternRule {
            id: id.to_string(),
            name: id.to_string(),
            jlpt: "N5".to_string(),
            steps,
            ..PatternRule::default()
        }
    }

    #[test]
    fn context_disambiguates_subject_ga_from_contrastive_ga() {
        let mut subject = rule("subject-ga", vec![surface("が")]);
        subject.ambiguity_group = Some("ga".to_string());
        subject.fallback = true;

        let contrast = PatternRule {
            id: "contrast-ga".to_string(),
            name: "contrast-ga".to_string(),
            jlpt: "N5".to_string(),
            ambiguity_group: Some("ga".to_string()),
            variants: vec![PatternVariant {
                id: "after-predicate".to_string(),
                core: vec![surface("が")],
                left_context: vec![pos1("動詞")],
                ..PatternVariant::default()
            }],
            ..PatternRule::default()
        };

        let noun_matches = match_all(
            &tokens(&[("雨", "名詞"), ("が", "助詞")]),
            &[subject.clone(), contrast.clone()],
        );
        assert_eq!(noun_matches.len(), 1);
        assert_eq!(noun_matches[0].rule_id, "subject-ga");

        let predicate_matches = match_all(
            &tokens(&[("降る", "動詞"), ("が", "助詞")]),
            &[subject, contrast],
        );
        assert_eq!(predicate_matches.len(), 1);
        assert_eq!(predicate_matches[0].rule_id, "contrast-ga");
        assert_eq!(
            (
                predicate_matches[0].token_start,
                predicate_matches[0].token_end
            ),
            (1, 1)
        );
    }

    #[test]
    fn sentence_final_mon_context_does_not_expand_core_span() {
        let final_mon = PatternRule {
            id: "final-mon".to_string(),
            name: "final-mon".to_string(),
            jlpt: "N3".to_string(),
            variants: vec![PatternVariant {
                id: "sentence-final".to_string(),
                core: vec![surface("もん")],
                left_context: vec![pos1("動詞")],
                right_context: vec![surface("。")],
                right_boundary: Some(Boundary::Sentence),
                ..PatternVariant::default()
            }],
            ..PatternRule::default()
        };
        let found = match_all(
            &tokens(&[("する", "動詞"), ("もん", "名詞"), ("。", "補助記号")]),
            &[final_mon],
        );

        assert_eq!(found.len(), 1);
        assert_eq!((found[0].token_start, found[0].token_end), (1, 1));
    }

    #[test]
    fn fully_backtracks_across_multiple_bounded_gaps() {
        let pattern = rule(
            "gaps",
            vec![
                surface("a"),
                wildcard(0, 2),
                surface("b"),
                wildcard(0, 2),
                surface("c"),
                surface("d"),
            ],
        );
        let found = match_all(
            &tokens(&[
                ("a", ""),
                ("x", ""),
                ("b", ""),
                ("y", ""),
                ("c", ""),
                ("d", ""),
            ]),
            &[pattern],
        );

        assert_eq!(found.len(), 1);
        assert_eq!((found[0].token_start, found[0].token_end), (0, 5));
    }

    #[test]
    fn gaps_do_not_cross_clause_boundaries() {
        let pattern = rule(
            "clause-gap",
            vec![surface("a"), wildcard(0, 4), surface("b")],
        );
        let found = match_all(
            &tokens(&[("a", ""), ("、", "補助記号"), ("b", "")]),
            &[pattern],
        );
        assert!(found.is_empty());
    }

    #[test]
    fn optional_and_one_of_steps_match_and_capture() {
        let pattern = rule(
            "optional-one-of",
            vec![
                Step {
                    surface: Some("お".to_string()),
                    optional: true,
                    ..Step::default()
                },
                Step {
                    one_of: vec![
                        TokenAlternative::Surface("茶".to_string()),
                        TokenAlternative::Surface("水".to_string()),
                    ],
                    capture: Some("drink".to_string()),
                    ..Step::default()
                },
            ],
        );

        let with_optional = match_all(
            &tokens(&[("お", ""), ("茶", "")]),
            std::slice::from_ref(&pattern),
        );
        let without_optional = match_all(&tokens(&[("水", "")]), &[pattern]);
        assert_eq!(
            (with_optional[0].token_start, with_optional[0].token_end),
            (0, 1)
        );
        assert_eq!(
            (
                without_optional[0].token_start,
                without_optional[0].token_end
            ),
            (0, 0)
        );
        assert_eq!(with_optional[0].captures[0].name, "drink");
        assert_eq!(
            (
                with_optional[0].captures[0].token_start,
                with_optional[0].captures[0].token_end
            ),
            (1, 1)
        );
    }

    #[test]
    fn unrelated_nested_matches_are_retained() {
        let found = match_all(
            &tokens(&[("a", ""), ("b", "")]),
            &[
                rule("outer", vec![surface("a"), surface("b")]),
                rule("inner", vec![surface("b")]),
            ],
        );

        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|matched| matched.rule_id == "outer"));
        assert!(found.iter().any(|matched| matched.rule_id == "inner"));
    }

    #[test]
    fn same_sense_nested_matches_are_retained() {
        let mut outer = rule("outer", vec![surface("a"), surface("b")]);
        outer.sense_id = Some("shared".to_string());
        let mut inner = rule("inner", vec![surface("b")]);
        inner.sense_id = Some("shared".to_string());

        let found = match_all(&tokens(&[("a", ""), ("b", "")]), &[outer, inner]);

        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|matched| matched.rule_id == "outer"));
        assert!(found.iter().any(|matched| matched.rule_id == "inner"));
    }

    #[test]
    fn equal_span_dedupes_senses_and_resolves_ambiguity_deterministically() {
        let mut broad = rule("broad", vec![surface("が")]);
        broad.sense_id = Some("subject".to_string());
        broad.priority = 100;

        let mut specific = rule(
            "specific",
            vec![Step {
                surface: Some("が".to_string()),
                pos1: Some("助詞".to_string()),
                ..Step::default()
            }],
        );
        specific.sense_id = Some("subject".to_string());

        let mut fallback = rule("fallback", vec![surface("が")]);
        fallback.ambiguity_group = Some("ga".to_string());
        fallback.fallback = true;

        specific.ambiguity_group = Some("ga".to_string());
        let found = match_all(&tokens(&[("が", "助詞")]), &[fallback, broad, specific]);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule_id, "specific");
    }

    #[test]
    fn clause_and_sentence_boundary_assertions_are_distinct() {
        let clause = PatternRule {
            id: "clause".to_string(),
            name: "clause".to_string(),
            jlpt: "N5".to_string(),
            variants: vec![PatternVariant {
                id: "after-comma".to_string(),
                core: vec![surface("x")],
                left_boundary: Some(Boundary::Clause),
                ..PatternVariant::default()
            }],
            ..PatternRule::default()
        };
        let sentence = PatternRule {
            id: "sentence".to_string(),
            name: "sentence".to_string(),
            jlpt: "N5".to_string(),
            variants: vec![PatternVariant {
                id: "after-period".to_string(),
                core: vec![surface("x")],
                left_boundary: Some(Boundary::Sentence),
                ..PatternVariant::default()
            }],
            ..PatternRule::default()
        };
        let after_comma = match_all(
            &tokens(&[("、", "補助記号"), ("x", "")]),
            &[clause.clone(), sentence.clone()],
        );
        let after_period = match_all(
            &tokens(&[("。", "補助記号"), ("x", "")]),
            &[clause, sentence],
        );

        assert_eq!(after_comma.len(), 1);
        assert_eq!(after_comma[0].rule_id, "clause");
        assert_eq!(after_period.len(), 2);
    }

    #[test]
    fn raw_candidates_keep_optional_paths_while_match_all_keeps_first_path() {
        let pattern = rule(
            "optional-paths",
            vec![
                surface("a"),
                Step {
                    surface: Some("b".to_string()),
                    optional: true,
                    ..Step::default()
                },
            ],
        );
        let input = tokens(&[("a", ""), ("b", "")]);

        let candidates = match_candidates(&input, std::slice::from_ref(&pattern));
        let resolved = match_all(&input, &[pattern]);

        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|candidate| {
            (candidate.matched.token_start, candidate.matched.token_end) == (0, 0)
        }));
        assert!(candidates.iter().any(|candidate| {
            (candidate.matched.token_start, candidate.matched.token_end) == (0, 1)
        }));
        assert_eq!((resolved[0].token_start, resolved[0].token_end), (0, 1));
    }

    #[test]
    fn raw_candidates_keep_wildcard_paths_while_match_all_keeps_min_first_path() {
        let pattern = rule(
            "wildcard-paths",
            vec![surface("a"), wildcard(0, 1), surface("b")],
        );
        let input = tokens(&[("a", ""), ("b", ""), ("b", "")]);

        let candidates = match_candidates(&input, std::slice::from_ref(&pattern));
        let resolved = match_all(&input, &[pattern]);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].wildcard_steps, 1);
        assert_eq!((resolved[0].token_start, resolved[0].token_end), (0, 1));
    }
}
