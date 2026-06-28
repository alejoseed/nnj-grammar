use crate::matcher::PatternMatch;
use crate::tokenizer::Token;

/// Terminal display columns for a string.
/// Japanese/CJK characters occupy 2 columns; everything else occupies 1.
fn cols(s: &str) -> usize {
    s.chars()
        .map(|c| {
            let n = c as u32;
            match n {
                0x3000..=0x9FFF   // Hiragana, Katakana, CJK unified ideographs
                | 0xAC00..=0xD7AF // Korean (just in case)
                | 0xF900..=0xFAFF // CJK compatibility ideographs
                | 0xFF00..=0xFFEF // Full-width forms
                => 2,
                _ => 1, // ASCII, box-drawing, arrows — all single-width
            }
        })
        .sum()
}

/// Print the analysis as a terminal graph.
///
/// Example output:
///
///   [私]──→[の]──→[名前]──→[は]──→[コタロー]──→[です]
///                             │
///                      ┌──────────────────────────┐
///                      │ は (topic) · N5           │
///                      │ topic marker              │
///                      └──────────────────────────┘
pub fn print_graph(tokens: &[Token], matches: &[PatternMatch]) {
    if tokens.is_empty() {
        println!("(empty)");
        return;
    }

    // Build the token chain and record the display column where each box starts.
    let mut chain = String::new();
    let mut box_start: Vec<usize> = Vec::new(); // display col of opening [ for each token
    let mut cursor: usize = 0;

    for (i, token) in tokens.iter().enumerate() {
        box_start.push(cursor);
        let label = format!("[{}]", token.surface);
        cursor += cols(&label);
        chain.push_str(&label);
        if i < tokens.len() - 1 {
            let arrow = "──→";
            cursor += cols(arrow);
            chain.push_str(arrow);
        }
    }

    println!("{}", chain);

    // Draw each pattern annotation below the chain.
    for m in matches {
        // Connector hangs one column inside the opening [ of the matched token.
        let connector_col = box_start[m.token_start] + 1;
        let pad = " ".repeat(connector_col);

        // Vertical connector
        println!("{}│", pad);

        // Content lines for the box
        let title = format!("{} · {}", m.rule_name, m.jlpt);
        let mut lines: Vec<&str> = vec![&title, &m.meaning_en];
        if let Some(ref hint) = m.hint {
            lines.push(hint);
        }

        let inner = lines.iter().map(|l| cols(l)).max().unwrap_or(10);

        // top border
        println!("{}┌{}┐", pad, "─".repeat(inner + 2));

        // content rows
        for l in &lines {
            let right_pad = inner.saturating_sub(cols(l));
            println!("{}│ {}{} │", pad, l, " ".repeat(right_pad));
        }

        // bottom border
        println!("{}└{}┘", pad, "─".repeat(inner + 2));
    }

    // If no matches, say so.
    if matches.is_empty() {
        println!();
        println!("  (no grammar patterns matched)");
    }
}
