//! Score the deterministic 文節 chunker against UD Japanese-GSD gold
//! `BunsetuBILabel` annotations.
//!
//! GSD tokenization (UniDic short units) may disagree with our lindera build,
//! so boundaries are compared as *character offsets* into the concatenated
//! token surfaces, not as token indices. A boundary is the offset where a
//! bunsetsu begins; offset 0 is excluded (both sides start a bunsetsu there by
//! construction).
//!
//! Usage:
//!   cargo run --release --bin chunker-eval -- data/ud-japanese-gsd/ja_gsd-ud-dev.conllu
//!   cargo run --release --bin chunker-eval -- --worst 10 data/ud-japanese-gsd/*.conllu

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use nnj_grammar::chunker;
use nnj_grammar::tokenizer::Tokenizer;

struct GoldSentence {
    /// Concatenated token surfaces — fed to our tokenizer so both sides see
    /// the identical character stream.
    text: String,
    /// Character offsets where a gold bunsetsu starts (offset 0 excluded).
    boundaries: BTreeSet<usize>,
}

fn parse_conllu(content: &str) -> Vec<GoldSentence> {
    let mut sentences = Vec::new();
    let mut text = String::new();
    let mut boundaries = BTreeSet::new();
    let mut offset = 0usize;

    let mut flush = |text: &mut String, boundaries: &mut BTreeSet<usize>, offset: &mut usize| {
        if !text.is_empty() {
            sentences.push(GoldSentence {
                text: std::mem::take(text),
                boundaries: std::mem::take(boundaries),
            });
        }
        *offset = 0;
    };

    for line in content.lines() {
        if line.is_empty() {
            flush(&mut text, &mut boundaries, &mut offset);
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let columns: Vec<&str> = line.split('\t').collect();
        if columns.len() < 10 || columns[0].parse::<usize>().is_err() {
            continue; // multi-word ranges and empty nodes
        }
        let surface = columns[1];
        let misc = columns[9];
        let is_begin = misc
            .split('|')
            .any(|field| field == "BunsetuBILabel=B");
        if is_begin && offset > 0 {
            boundaries.insert(offset);
        }
        text.push_str(surface);
        offset += surface.chars().count();
    }
    flush(&mut text, &mut boundaries, &mut offset);
    sentences
}

/// Character offsets where our chunker starts a bunsetsu (offset 0 excluded).
fn predicted_boundaries(tokenizer: &Tokenizer, text: &str) -> Result<BTreeSet<usize>> {
    let tokens = tokenizer.tokenize(text)?;
    // Token positions -> character offsets.
    let mut char_starts = Vec::with_capacity(tokens.len());
    let mut offset = 0usize;
    for token in &tokens {
        char_starts.push(offset);
        offset += token.surface.chars().count();
    }
    let mut boundaries = BTreeSet::new();
    for sentence in chunker::chunk(&tokens) {
        for chunk in &sentence.bunsetsu {
            let start = char_starts[chunk.token_start];
            if start > 0 {
                boundaries.insert(start);
            }
        }
    }
    Ok(boundaries)
}

#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "boundary counts are far below 2^52"
)]
fn ratio(numerator: usize, denominator: usize) -> f64 {
    numerator as f64 / denominator as f64
}

struct Scored {
    text: String,
    f1: f64,
    gold: BTreeSet<usize>,
    predicted: BTreeSet<usize>,
}

fn main() -> Result<()> {
    let mut worst_count = 0usize;
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--worst" {
            worst_count = args
                .next()
                .context("--worst needs a number")?
                .parse()
                .context("--worst needs a number")?;
        } else {
            paths.push(arg);
        }
    }
    anyhow::ensure!(!paths.is_empty(), "usage: chunker-eval [--worst N] <conllu files>");

    let tokenizer = Tokenizer::new()?;
    let mut scored = Vec::new();

    for path in &paths {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
        let sentences = parse_conllu(&content);

        let mut true_positive = 0usize;
        let mut predicted_total = 0usize;
        let mut gold_total = 0usize;

        for gold in &sentences {
            let predicted = predicted_boundaries(&tokenizer, &gold.text)?;
            let hits = predicted.intersection(&gold.boundaries).count();
            true_positive += hits;
            predicted_total += predicted.len();
            gold_total += gold.boundaries.len();

            if worst_count > 0 {
                // Both empty means a single-bunsetsu sentence chunked
                // perfectly, not a zero score.
                let f1 = if predicted.is_empty() && gold.boundaries.is_empty() {
                    1.0
                } else {
                    let precision = ratio(hits, predicted.len().max(1));
                    let recall = ratio(hits, gold.boundaries.len().max(1));
                    if precision + recall == 0.0 {
                        0.0
                    } else {
                        2.0 * precision * recall / (precision + recall)
                    }
                };
                scored.push(Scored {
                    text: gold.text.clone(),
                    f1,
                    gold: gold.boundaries.clone(),
                    predicted,
                });
            }
        }

        let precision = ratio(true_positive, predicted_total.max(1));
        let recall = ratio(true_positive, gold_total.max(1));
        let f1 = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };
        println!(
            "{path}: sentences={} P={:.4} R={:.4} F1={:.4}",
            sentences.len(),
            precision,
            recall,
            f1
        );
    }

    if worst_count > 0 {
        scored.sort_by(|a, b| a.f1.total_cmp(&b.f1));
        println!("\nworst {} sentences:", worst_count.min(scored.len()));
        for item in scored.iter().take(worst_count) {
            println!("\nF1={:.3} {}", item.f1, item.text);
            let missed: Vec<_> = item.gold.difference(&item.predicted).collect();
            let spurious: Vec<_> = item.predicted.difference(&item.gold).collect();
            println!("  missed splits at: {missed:?}");
            println!("  spurious splits at: {spurious:?}");
        }
    }
    Ok(())
}
