//! Deterministic 文 (sentence) and 文節 (bunsetsu) chunking over UniDic tokens.
//!
//! This is the structural layer of the analysis: the graph's shape comes from
//! here, not from which grammar rules matched. A bunsetsu is one content word
//! (自立語) plus its trailing function words (付属語: 助詞, 助動詞, 接尾辞,
//! auxiliary uses of 非自立可能 verbs/adjectives). Function words are a closed
//! class and UniDic tags them all, so no grammar catalog is consulted.
//!
//! Conventions (Kyoto-corpus style, chosen so auxiliary chains stay with their
//! host — 知ってる and 食べてはいけない are each one bunsetsu):
//!   - punctuation attaches to the preceding bunsetsu (no floating 、。……)
//!   - an opening bracket 括弧開 binds forward: 「はい」と is one bunsetsu
//!   - a 非自立可能 verb/adjective attaches only when it actually follows a
//!     て/で chain (optionally through the focus particles は/も: てはいけない,
//!     てもいい). UniDic sets 非自立可能 on lexical *potential*, not usage —
//!     行き in 行きました！ carries the tag while being the main verb.

use serde::Serialize;

use crate::tokenizer::Token;

/// One 文節: an inclusive token span. Spans within a sentence are contiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Bunsetsu {
    pub token_start: usize,
    pub token_end: usize,
}

/// One 文, ending at a 句点 run (。！？ plus trailing closing brackets) or at
/// the end of input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SentenceChunk {
    pub token_start: usize,
    pub token_end: usize,
    pub bunsetsu: Vec<Bunsetsu>,
}

/// Split tokens into 文 and each 文 into 文節. Every token belongs to exactly
/// one bunsetsu of exactly one sentence.
pub fn chunk(tokens: &[Token]) -> Vec<SentenceChunk> {
    split_sentences(tokens)
        .into_iter()
        .map(|(start, end)| SentenceChunk {
            token_start: start,
            token_end: end,
            bunsetsu: chunk_bunsetsu(tokens, start, end),
        })
        .collect()
}

/// Inclusive (start, end) token ranges for each 文.
fn split_sentences(tokens: &[Token]) -> Vec<(usize, usize)> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut position = 0;
    while position < tokens.len() {
        if is_sentence_terminator(&tokens[position]) {
            // Absorb the whole terminator run plus closing brackets: 。」 !?
            let mut end = position;
            while end + 1 < tokens.len()
                && (is_sentence_terminator(&tokens[end + 1]) || is_closing_bracket(&tokens[end + 1]))
            {
                end += 1;
            }
            sentences.push((start, end));
            start = end + 1;
            position = end + 1;
        } else {
            position += 1;
        }
    }
    if start < tokens.len() {
        sentences.push((start, tokens.len() - 1));
    }
    sentences
}

fn is_sentence_terminator(token: &Token) -> bool {
    // Surface fallback: GSD web text uses ASCII !? which lindera sometimes
    // tags as something other than 補助記号.
    (token.pos1 == "補助記号" && token.pos2 == "句点")
        || matches!(token.surface.as_str(), "!" | "?")
}

/// Single-character punctuation by surface, regardless of POS. Halfwidth
/// punctuation in web text (, . -) is frequently mis-tagged as 名詞 by the
/// tokenizer; classifying by surface keeps it attached to the preceding
/// bunsetsu instead of letting it seed a noun compound. Brackets are excluded
/// (they bind forward); ー is excluded (it is a vowel mark, not punctuation).
fn is_punct_surface(token: &Token) -> bool {
    let mut chars = token.surface.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return false;
    };
    if matches!(c, '(' | '[' | '{') {
        return false;
    }
    c.is_ascii_punctuation() || matches!(c, '、' | '。' | '・' | '…' | '‥' | '“' | '”' | '※')
}

fn is_closing_bracket(token: &Token) -> bool {
    token.pos2 == "括弧閉"
}

/// One chunking decision with its justification, for the trace output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Decision {
    /// Token position this decision is about.
    pub position: usize,
    /// true = this token opens a new bunsetsu; false = it attaches.
    pub starts_new: bool,
    /// Which rule in `starts_new_bunsetsu` decided, in plain words.
    pub reason: &'static str,
}

/// Re-run the chunker and report why every token attached or split. Uses the
/// exact same decision function as `chunk`, so the trace can never drift from
/// real behavior.
pub fn trace(tokens: &[Token]) -> Vec<Decision> {
    let mut decisions = Vec::with_capacity(tokens.len());
    for (start, end) in split_sentences(tokens) {
        decisions.push(Decision {
            position: start,
            starts_new: true,
            reason: "文頭 — the first token of a sentence always opens a bunsetsu",
        });
        for position in start + 1..=end {
            let (starts_new, reason) = starts_new_bunsetsu(tokens, position);
            decisions.push(Decision {
                position,
                starts_new,
                reason,
            });
        }
    }
    decisions
}

/// Chunk one sentence's tokens (inclusive range) into bunsetsu.
fn chunk_bunsetsu(tokens: &[Token], start: usize, end: usize) -> Vec<Bunsetsu> {
    let mut chunks: Vec<Bunsetsu> = Vec::new();
    for position in start..=end {
        let open = chunks.last_mut();
        match open {
            Some(current) if !starts_new_bunsetsu(tokens, position).0 => {
                current.token_end = position;
            }
            _ => chunks.push(Bunsetsu {
                token_start: position,
                token_end: position,
            }),
        }
    }
    chunks
}

/// Does the token at `position` open a new bunsetsu, and under which rule?
/// Callers guarantee a bunsetsu is already open (the first token of a
/// sentence always opens one).
fn starts_new_bunsetsu(tokens: &[Token], position: usize) -> (bool, &'static str) {
    let token = &tokens[position];
    let previous = &tokens[position - 1];

    // Everything binds to a preceding prefix or opening bracket.
    if previous.pos1 == "接頭辞" || previous.pos2 == "括弧開" {
        return (
            false,
            "previous token is 接頭辞/括弧開 — prefixes and opening brackets bind forward",
        );
    }
    // Punctuation attaches, whatever the tokenizer thinks it is.
    if is_punct_surface(token) {
        return if token.pos2 == "括弧開" {
            (true, "opening bracket — binds to what follows, not what precedes")
        } else {
            (false, "punctuation (by surface) attaches to the preceding bunsetsu")
        };
    }

    match token.pos1.as_str() {
        // 付属語 always attach.
        "助詞" => (false, "助詞 (particle) is 付属語 — attaches to its host"),
        "助動詞" => (false, "助動詞 (auxiliary) is 付属語 — attaches to its host"),
        "接尾辞" => (false, "接尾辞 (suffix) is 付属語 — attaches to its host"),
        "補助記号" | "記号" => {
            if token.pos2 == "括弧開" {
                (true, "opening bracket — binds to what follows, not what precedes")
            } else {
                (false, "punctuation/symbol attaches to the preceding bunsetsu")
            }
        }
        // Compound nouns stay together (UniDic short units split 日本語 into
        // 日本+語, 文化財 into 文化+財).
        "名詞" => {
            // A proper noun after a geographic suffix starts a new name:
            // いなべ市|藤原町. (Not after arbitrary common nouns — 元広島 and
            // 日本語 are compounds.)
            let new_place = token.pos2 == "固有名詞"
                && matches!(
                    previous.surface.as_str(),
                    "市" | "町" | "村" | "区" | "郡" | "県" | "都" | "道" | "府" | "州"
                );
            if new_place {
                (true, "固有名詞 after a geographic suffix — a new place name, not a compound")
            } else if compounds_with_following_noun(previous) {
                (false, "noun after a noun/nominal suffix/・ — continues the compound")
            } else {
                (true, "名詞 is 自立語 (content word) — opens a new bunsetsu")
            }
        }
        // 助動詞語幹 (そう, よう, みたい) bind to the predicate stem they
        // follow: イきそう, しちゃいそう. After a particle they head their own
        // bunsetsu (関係者の|ようでした). 形状詞 also continue noun compounds
        // (重要文化財); standalone 形状詞 (ドロドロ, 静か) start their own.
        "形状詞" if token.pos2 == "助動詞語幹" => {
            if matches!(previous.pos1.as_str(), "動詞" | "形容詞" | "助動詞") {
                (false, "助動詞語幹 (そう/よう/みたい) binds to the predicate stem before it")
            } else {
                (true, "助動詞語幹 after a non-predicate — heads its own bunsetsu")
            }
        }
        "形状詞" => {
            if compounds_with_following_noun(previous) {
                (false, "形状詞 after a noun — continues the compound (重要文化財)")
            } else {
                (true, "形状詞 is 自立語 (content word) — opens a new bunsetsu")
            }
        }
        // サ変 verbal nouns bind their する: 火傷し, 勉強する.
        "動詞" if token.base_form == "為る" && previous.pos3.contains("サ変") => {
            (false, "する after a サ変可能 noun — verbal-noun compound (勉強する)")
        }
        // Auxiliary use of a 非自立可能 verb/adjective: after a て/で chain
        // (知ってる, 食べてはいけない) or as the second half of a compound
        // predicate on a 連用形/語幹 stem (強すぎる, 走り出す). Everything
        // else — including 非自立可能-tagged main verbs — starts a new
        // bunsetsu.
        "動詞" | "形容詞" if token.pos2 == "非自立可能" => {
            if follows_te_chain(tokens, position) {
                (false, "非自立可能 after a て/で chain — auxiliary use (ている, てはいけない)")
            } else if is_stem(previous) {
                (false, "非自立可能 on a 連用形/語幹 stem — compound predicate (強すぎる)")
            } else {
                (true, "非自立可能 without て/stem before it — main-verb use, opens a bunsetsu")
            }
        }
        _ => (true, "自立語 (content word) — opens a new bunsetsu"),
    }
}

/// A verb/adjective form that hosts a compound predicate: 連用形 (走り出す)
/// or 語幹 (強すぎる).
fn is_stem(token: &Token) -> bool {
    matches!(token.pos1.as_str(), "動詞" | "形容詞")
        && (token.conj_form.starts_with("連用形") || token.conj_form.starts_with("語幹"))
}

/// Can `previous` host a following noun/形状詞 in the same compound?
/// Nouns and nominal suffixes (文化+財+…) do; ・ joins list compounds
/// (マダガスカル・コモロ); adverbial nouns (毎日, 昨日) and everything else —
/// including other punctuation — do not.
fn compounds_with_following_noun(previous: &Token) -> bool {
    if previous.surface == "・" {
        return true;
    }
    if is_punct_surface(previous) {
        return false;
    }
    match previous.pos1.as_str() {
        "名詞" => !previous.pos3.contains("副詞可能"),
        "接尾辞" => previous.pos2 == "名詞的",
        _ => false,
    }
}

/// True when the tokens before `position`, skipping the focus particles は/も
/// (係助詞), end with て or で — the conjunctive particle (知ってる,
/// 食べてはいけない) or the copula's 連用形 (である).
fn follows_te_chain(tokens: &[Token], position: usize) -> bool {
    let mut index = position;
    while index > 0 {
        index -= 1;
        let token = &tokens[index];
        if token.pos2 == "係助詞" && (token.surface == "は" || token.surface == "も") {
            continue;
        }
        return (token.pos2 == "接続助詞" || token.pos1 == "助動詞")
            && (token.surface == "て" || token.surface == "で");
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::Tokenizer;

    /// Render chunks as nested surface strings for readable assertions.
    fn chunk_surfaces(tokenizer: &Tokenizer, text: &str) -> Vec<Vec<String>> {
        let tokens = tokenizer.tokenize(text).expect("tokenize");
        chunk(&tokens)
            .iter()
            .map(|sentence| {
                sentence
                    .bunsetsu
                    .iter()
                    .map(|chunk| {
                        tokens[chunk.token_start..=chunk.token_end]
                            .iter()
                            .map(|token| token.surface.as_str())
                            .collect()
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn chunks_issue_bank_and_particle_host_sentences() {
        let tokenizer = Tokenizer::new().expect("tokenizer");
        let cases: &[(&str, &[&[&str]])] = &[
            // The original bug: one bunsetsu, host + particle together.
            ("わたしは", &[&["わたしは"]]),
            // Noun compounding and per-noun splits.
            ("わたしの友達も来た", &[&["わたしの", "友達も", "来た"]]),
            // て-chain auxiliaries merge, even through は/も.
            ("食べてはいけない", &[&["食べてはいけない"]]),
            ("知ってる", &[&["知ってる"]]),
            // 非自立可能 as a main verb still starts its own bunsetsu.
            ("東京には行った", &[&["東京には", "行った"]]),
            // Relative clause stays split at this layer (NP grouping is 節 work).
            (
                "昨日買った本は面白い",
                &[&["昨日", "買った", "本は", "面白い"]],
            ),
            ("わたしは本も読む", &[&["わたしは", "本も", "読む"]]),
            ("そしてなによりも", &[&["そして", "なによりも"]]),
            // Sentence splitting on 。！？ with brackets and ellipses attached.
            (
                "うん、知ってる……んんっ。だって、わたしの中、ドロドロだもん。",
                &[
                    &["うん、", "知ってる……", "んんっ。"],
                    &["だって、", "わたしの", "中、", "ドロドロだもん。"],
                ],
            ),
            (
                "行きました！本当？「はい」と言った",
                &[&["行きました！"], &["本当？"], &["「はい」と", "言った"]],
            ),
        ];

        for (text, expected) in cases {
            let actual = chunk_surfaces(&tokenizer, text);
            let expected: Vec<Vec<String>> = expected
                .iter()
                .map(|sentence| sentence.iter().map(|s| s.to_string()).collect())
                .collect();
            assert_eq!(actual, expected, "chunking mismatch for {text:?}");
        }
    }

    #[test]
    fn every_token_is_covered_exactly_once() {
        let tokenizer = Tokenizer::new().expect("tokenizer");
        let text = "イきそうなら、いつでもイってくれていいですからね」オレも、イきましたよ、すっごいイきました";
        let tokens = tokenizer.tokenize(text).expect("tokenize");
        let sentences = chunk(&tokens);

        let mut covered = Vec::new();
        for sentence in &sentences {
            for chunk in &sentence.bunsetsu {
                assert!(chunk.token_start <= chunk.token_end);
                covered.extend(chunk.token_start..=chunk.token_end);
            }
        }
        let expected: Vec<usize> = (0..tokens.len()).collect();
        assert_eq!(covered, expected, "gaps or overlaps in bunsetsu coverage");
    }

    #[test]
    fn empty_input_yields_no_sentences() {
        assert!(chunk(&[]).is_empty());
    }
}
