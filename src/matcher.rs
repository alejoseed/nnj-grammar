use crate::patterns::PatternRule;
use crate::tokenizer::Token;

/// One grammar construction found in the token stream.
#[derive(Debug, Clone)]
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
            if let Some(end) = try_match(tokens, start, &rule.steps, 0) {
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

/// Try to match `steps[step_idx..]` starting at `tokens[token_pos]`.
///
/// Returns `Some(last_token_index)` on success, `None` on failure.
///
/// This is the recursive heart of the matcher — implement it.
fn try_match(
    tokens: &[Token],
    token_pos: usize,
    steps: &[crate::patterns::Step],
    step_idx: usize,
) -> Option<usize> {
    // All steps satisfied — return the index of the last consumed token.
    // token_pos here is the next position to consume, so the last consumed
    // token is token_pos - 1.  Handle the edge case where nothing was consumed.
    if step_idx >= steps.len() {
        return if token_pos > 0 { Some(token_pos - 1) } else { None };
    }

    // Out of tokens but still have steps to satisfy.
    if token_pos >= tokens.len() {
        return None;
    }

    let step = &steps[step_idx];

    if let Some(ref w) = step.wildcard {
        // TODO: implement wildcard matching
        // Try k = w.min, w.min+1, ..., w.max (ascending — NOT greedy)
        // For each k, call try_match(tokens, token_pos + k, steps, step_idx + 1)
        // Return the first Some result.
        let _ = w; // suppress unused warning until you implement this
        todo!("implement wildcard step matching")
    } else {
        // TODO: implement non-wildcard step matching
        // If step.matches(&tokens[token_pos]) is true, recurse into the next step.
        // Otherwise return None.
        todo!("implement token step matching")
    }
}
