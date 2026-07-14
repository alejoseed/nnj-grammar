use std::cmp::Ordering;
use std::collections::HashMap;

use serde::Serialize;

use crate::matcher::{MatchCandidate, PatternCapture};
use crate::patterns::CatalogSource;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchScore {
    pub fallback: bool,
    pub priority: i32,
    pub span_length: usize,
    pub core_specificity: usize,
    pub context_specificity: usize,
    pub wildcard_steps: usize,
    pub optional_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchProvenance {
    pub source: CatalogSource,
    pub rule_id: String,
    pub variant_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DisplayMatch {
    pub id: String,
    pub rule_name: String,
    pub jlpt: String,
    pub meaning_en: String,
    pub hint: Option<String>,
    pub sense_id: Option<String>,
    pub ambiguity_group: Option<String>,
    pub captures: Vec<PatternCapture>,
    pub token_start: usize,
    pub token_end: usize,
    pub score: MatchScore,
    pub provenance: Vec<MatchProvenance>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SecondaryReason {
    ContainedByStrongerMatch,
    OverlapsStrongerMatch,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecondaryMatch {
    pub id: String,
    pub matched: DisplayMatch,
    pub reason: SecondaryReason,
    pub blocked_by: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RankedMatches {
    pub primary: Vec<DisplayMatch>,
    pub secondary: Vec<SecondaryMatch>,
}

pub fn rank_candidates(mut candidates: Vec<MatchCandidate>) -> RankedMatches {
    candidates.sort_by(candidate_rank_order);
    let mut grouped = group_exact_duplicates(candidates);
    assign_stable_ids(&mut grouped);

    let mut primary: Vec<DisplayMatch> = Vec::new();
    let mut secondary = Vec::new();
    for matched in grouped {
        if let Some(blocker) = primary.iter().find(|existing| overlaps(existing, &matched)) {
            let reason = if contains(blocker, &matched) {
                SecondaryReason::ContainedByStrongerMatch
            } else {
                SecondaryReason::OverlapsStrongerMatch
            };
            secondary.push(SecondaryMatch {
                id: String::new(),
                matched,
                reason,
                blocked_by: Some(blocker.id.clone()),
            });
        } else {
            primary.push(matched);
        }
    }

    primary.sort_by(display_source_order);
    secondary.sort_by(|left, right| display_source_order(&left.matched, &right.matched));
    assign_secondary_ids(&mut secondary);
    RankedMatches { primary, secondary }
}

fn assign_secondary_ids(matches: &mut [SecondaryMatch]) {
    let mut span_counts = HashMap::<(usize, usize), usize>::new();
    for secondary in matches {
        let matched = &secondary.matched;
        let index = span_counts
            .entry((matched.token_start, matched.token_end))
            .or_default();
        secondary.id = format!(
            "secondary-{}-{}-{index}",
            matched.token_start, matched.token_end
        );
        *index += 1;
    }
}

fn candidate_rank_order(left: &MatchCandidate, right: &MatchCandidate) -> Ordering {
    left.fallback
        .cmp(&right.fallback)
        .then_with(|| right.priority.cmp(&left.priority))
        .then_with(|| span_length(right).cmp(&span_length(left)))
        .then_with(|| right.core_specificity.cmp(&left.core_specificity))
        .then_with(|| right.context_specificity.cmp(&left.context_specificity))
        .then_with(|| left.wildcard_steps.cmp(&right.wildcard_steps))
        .then_with(|| left.optional_steps.cmp(&right.optional_steps))
        .then_with(|| left.matched.rule_id.cmp(&right.matched.rule_id))
        .then_with(|| left.matched.variant_id.cmp(&right.matched.variant_id))
        .then_with(|| left.matched.token_start.cmp(&right.matched.token_start))
        .then_with(|| left.matched.token_end.cmp(&right.matched.token_end))
}

fn span_length(candidate: &MatchCandidate) -> usize {
    candidate.matched.token_end - candidate.matched.token_start + 1
}

fn group_exact_duplicates(candidates: Vec<MatchCandidate>) -> Vec<DisplayMatch> {
    let mut groups: Vec<((usize, usize, String, String), DisplayMatch)> = Vec::new();
    for candidate in candidates {
        let fingerprint = (
            candidate.matched.token_start,
            candidate.matched.token_end,
            normalize(&candidate.matched.rule_name),
            normalize(&candidate.matched.meaning_en),
        );
        let score = MatchScore {
            fallback: candidate.fallback,
            priority: candidate.priority,
            span_length: span_length(&candidate),
            core_specificity: candidate.core_specificity,
            context_specificity: candidate.context_specificity,
            wildcard_steps: candidate.wildcard_steps,
            optional_steps: candidate.optional_steps,
        };
        let provenance = MatchProvenance {
            source: candidate.matched.source.clone(),
            rule_id: candidate.matched.rule_id.clone(),
            variant_id: candidate.matched.variant_id.clone(),
        };
        if let Some((_, existing)) = groups.iter_mut().find(|(key, _)| key == &fingerprint) {
            if !existing.provenance.contains(&provenance) {
                existing.provenance.push(provenance);
                existing.provenance.sort_by(provenance_order);
            }
            continue;
        }

        groups.push((
            fingerprint,
            DisplayMatch {
                id: String::new(),
                rule_name: candidate.matched.rule_name,
                jlpt: candidate.matched.jlpt,
                meaning_en: candidate.matched.meaning_en,
                hint: candidate.matched.hint,
                sense_id: candidate.matched.sense_id,
                ambiguity_group: candidate.matched.ambiguity_group,
                captures: candidate.matched.captures,
                token_start: candidate.matched.token_start,
                token_end: candidate.matched.token_end,
                score,
                provenance: vec![provenance],
            },
        ));
    }
    groups.into_iter().map(|(_, matched)| matched).collect()
}

fn assign_stable_ids(matches: &mut [DisplayMatch]) {
    let mut span_counts = HashMap::<(usize, usize), usize>::new();
    for matched in matches {
        let index = span_counts
            .entry((matched.token_start, matched.token_end))
            .or_default();
        matched.id = if *index == 0 {
            format!("match-{}-{}", matched.token_start, matched.token_end)
        } else {
            format!(
                "match-{}-{}-{index}",
                matched.token_start, matched.token_end
            )
        };
        *index += 1;
    }
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn provenance_order(left: &MatchProvenance, right: &MatchProvenance) -> Ordering {
    (&left.source.id, &left.rule_id, &left.variant_id).cmp(&(
        &right.source.id,
        &right.rule_id,
        &right.variant_id,
    ))
}

fn overlaps(left: &DisplayMatch, right: &DisplayMatch) -> bool {
    left.token_start <= right.token_end && right.token_start <= left.token_end
}

fn contains(container: &DisplayMatch, contained: &DisplayMatch) -> bool {
    container.token_start <= contained.token_start && container.token_end >= contained.token_end
}

fn display_source_order(left: &DisplayMatch, right: &DisplayMatch) -> Ordering {
    (left.token_start, left.token_end, &left.id).cmp(&(
        right.token_start,
        right.token_end,
        &right.id,
    ))
}
