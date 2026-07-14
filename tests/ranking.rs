use nnj_grammar::matcher::{MatchCandidate, PatternMatch};
use nnj_grammar::patterns::CatalogSource;
use nnj_grammar::ranking::{rank_candidates, SecondaryReason};

#[derive(Clone, Copy)]
struct Evidence {
    fallback: bool,
    priority: i32,
    core_specificity: usize,
    context_specificity: usize,
    wildcard_steps: usize,
    optional_steps: usize,
}

impl Default for Evidence {
    fn default() -> Self {
        Self {
            fallback: false,
            priority: 0,
            core_specificity: 1,
            context_specificity: 0,
            wildcard_steps: 0,
            optional_steps: 0,
        }
    }
}

fn candidate(
    rule_id: &str,
    source_id: &str,
    name: &str,
    meaning: &str,
    span: (usize, usize),
    evidence: Evidence,
) -> MatchCandidate {
    MatchCandidate {
        matched: PatternMatch {
            rule_id: rule_id.to_string(),
            variant_id: "default".to_string(),
            rule_name: name.to_string(),
            jlpt: "N5".to_string(),
            meaning_en: meaning.to_string(),
            hint: None,
            sense_id: Some(rule_id.to_string()),
            ambiguity_group: None,
            source: CatalogSource::new(source_id, source_id),
            captures: Vec::new(),
            token_start: span.0,
            token_end: span.1,
        },
        fallback: evidence.fallback,
        priority: evidence.priority,
        core_specificity: evidence.core_specificity,
        context_specificity: evidence.context_specificity,
        wildcard_steps: evidence.wildcard_steps,
        optional_steps: evidence.optional_steps,
        discovery_order: 0,
    }
}

#[test]
fn longer_specific_match_contains_broad_match_as_secondary() {
    let ranked = rank_candidates(vec![
        candidate(
            "broad-mo",
            "bunpro-local",
            "誰か・どこか・誰も・どこも",
            "indefinite pronoun",
            (3, 3),
            Evidence::default(),
        ),
        candidate(
            "nani-yori",
            "bunpro-local",
            "何より",
            "Above all",
            (1, 3),
            Evidence::default(),
        ),
    ]);

    assert_eq!(ranked.primary.len(), 1);
    assert_eq!(ranked.primary[0].rule_name, "何より");
    assert_eq!(ranked.secondary.len(), 1);
    assert_eq!(
        ranked.secondary[0].reason,
        SecondaryReason::ContainedByStrongerMatch
    );
    assert_eq!(
        ranked.secondary[0].blocked_by.as_deref(),
        Some(ranked.primary[0].id.as_str())
    );
}

#[test]
fn non_overlapping_primary_matches_are_returned_in_source_order() {
    let ranked = rank_candidates(vec![
        candidate(
            "later",
            "local",
            "later",
            "later",
            (2, 3),
            Evidence {
                priority: 100,
                ..Evidence::default()
            },
        ),
        candidate(
            "earlier",
            "hanabira",
            "earlier",
            "earlier",
            (0, 0),
            Evidence::default(),
        ),
    ]);

    assert_eq!(
        ranked
            .primary
            .iter()
            .map(|matched| matched.rule_name.as_str())
            .collect::<Vec<_>>(),
        ["earlier", "later"]
    );
}

#[test]
fn exact_display_duplicates_group_provenance() {
    let ranked = rank_candidates(vec![
        candidate(
            "hanabira-rule",
            "hanabira",
            "そして",
            "And then",
            (0, 0),
            Evidence::default(),
        ),
        candidate(
            "local-rule",
            "bunpro-local",
            "そして",
            "And then",
            (0, 0),
            Evidence::default(),
        ),
    ]);

    assert_eq!(ranked.primary.len(), 1);
    assert_eq!(ranked.primary[0].provenance.len(), 2);
    assert!(ranked.secondary.is_empty());
}

#[test]
fn different_meanings_on_the_same_span_remain_distinct() {
    let ranked = rank_candidates(vec![
        candidate(
            "strong",
            "local",
            "か",
            "question",
            (1, 1),
            Evidence {
                priority: 10,
                ..Evidence::default()
            },
        ),
        candidate("weak", "local", "か", "or", (1, 1), Evidence::default()),
    ]);

    assert_eq!(ranked.primary[0].meaning_en, "question");
    assert_eq!(ranked.secondary[0].matched.meaning_en, "or");
}

#[test]
fn ranking_is_independent_of_candidate_input_order() {
    let forward = vec![
        candidate("short", "local", "も", "also", (3, 3), Evidence::default()),
        candidate(
            "long",
            "local",
            "何より",
            "Above all",
            (1, 3),
            Evidence::default(),
        ),
    ];
    let mut reversed = forward.clone();
    reversed.reverse();

    assert_eq!(
        serde_json::to_string(&rank_candidates(forward)).unwrap(),
        serde_json::to_string(&rank_candidates(reversed)).unwrap()
    );
}
