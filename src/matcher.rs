use crate::patterns::PatternRule;
use crate::tokenizer::Token;

/// One grammar construction found in the token stream.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PatternMatch {
    pub rule_id: String,
    pub rule_name: String,
    pub jlpt: String,
    pub meaning_en: String,
    pub hint: Option<String>,
    /// Index of the first token that belongs to this match.
    pub token_start: usize,
    /// Index of the last token that belongs to this match (inclusive).
    pub token_end: usize,
}

/// Find all grammar construction matches in `tokens` against `rules`.
///
/// Returns all overlapping matches sorted by `token_start`.
///
/// ## Your job
///
/// Implement the sliding-window POS-sequence matching algorithm here.
///
/// For each rule:
///   1. Try starting a match at every token position i.
///   2. Walk through `rule.steps` in order.
///   3. For a non-wildcard step: call `step.matches(&tokens[j])` — if it
///      matches, advance j by 1; if not, the attempt at position i fails.
///   4. For a wildcard step (step.wildcard is Some(w)): try consuming k tokens
///      for k from w.min to w.max **in ascending order** (NOT greedy / not max-
///      first). For each k, recursively try to satisfy the remaining steps
///      starting at j+k. Return success on the first k that works.
///      (Greedy / max-first breaks span patterns like しか〜ない.)
///   5. If all steps are satisfied, record a PatternMatch.
///
/// After collecting all matches, sort by token_start and return.
pub fn match_all(tokens: &[Token], rules: &[PatternRule]) -> Vec<PatternMatch> {
    let mut matches: Vec<PatternMatch> = Vec::new();

    for rule in rules {
        for start in 0..tokens.len() {
            if let Some(end) = try_match(tokens, start, &rule.steps) {
                matches.push(PatternMatch {
                    rule_id: rule.id.clone(),
                    rule_name: rule.name.clone(),
                    jlpt: rule.jlpt.clone(),
                    meaning_en: rule.meaning_en.clone(),
                    hint: rule.hint.clone(),
                    token_start: start,
                    token_end: end,
                });
            }
        }
    }

    matches.sort_by_key(|m| m.token_start);
    matches
}

/// Walk through `steps` starting at `tokens[start]`.
/// Returns the index of the last matched token on success, None on failure.
fn try_match(tokens: &[Token], start: usize, steps: &[crate::patterns::Step]) -> Option<usize> {
    let mut pos = start;
    
    for (i, step) in steps.iter().enumerate() {
        if let Some(w) = &step.wildcard {
            // Wildcard: try consuming k tokens (ascending, not greedy) then
            // check if the remai ning steps fit from that position.
            let suffix = &steps[i + 1..];
            for k in w.min..=w.max {
                let after = pos + k;
                if after > tokens.len() { break; }
                if let Some(end) = match_tail(tokens, after, suffix) {
                    return Some(end);
                }
            }
            return None;
        }

        if pos >= tokens.len() { return None; }
        if !step.matches(&tokens[pos]) { return None; }
        pos += 1;
    }

    // All steps consumed. pos is one past the last matched token.
    (pos > start).then_some(pos - 1)
}

/// Match a sequence of non-wildcard steps starting at `tokens[start]`.
/// Called for the suffix after a wildcard — wildcards are not expected here.
fn match_tail(tokens: &[Token], start: usize, steps: &[crate::patterns::Step]) -> Option<usize> {
    if steps.is_empty() {
        // Wildcard was the last step. Last consumed token is start - 1.
        return start.checked_sub(1);
    }
    let mut pos = start;
    for step in steps {
        if pos >= tokens.len() { return None; }
        if !step.matches(&tokens[pos]) { return None; }
        pos += 1;
    }
    Some(pos - 1)
}
