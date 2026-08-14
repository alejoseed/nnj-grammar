use nnj_grammar::analysis::{AnalysisDocument, TreeNode, TreeNodeKind};
use nnj_grammar::chunker::SentenceChunk;
use nnj_grammar::matcher::PatternMatch;
use nnj_grammar::tokenizer::Token;

/// Print every chunking decision with the rule that made it. This is the
/// learning/debugging view of `src/chunker.rs::starts_new_bunsetsu`: read the
/// reason column, then find the matching arm in that function.
pub fn print_bunsetsu_trace(tokens: &[Token], decisions: &[nnj_grammar::chunker::Decision]) {
    for decision in decisions {
        let token = &tokens[decision.position];
        let pos = if token.pos2.is_empty() {
            token.pos1.clone()
        } else {
            format!("{}·{}", token.pos1, token.pos2)
        };
        let marker = if decision.starts_new { "┌ NEW   " } else { "│ attach" };
        println!(
            "{} {:<8} {:<18} {}",
            marker, token.surface, pos, decision.reason
        );
    }
}

/// Print 文/文節 chunking: one line per sentence, each bunsetsu bracketed with
/// its token pieces separated by `·`.
pub fn print_bunsetsu(tokens: &[Token], sentences: &[SentenceChunk]) {
    for (index, sentence) in sentences.iter().enumerate() {
        let rendered: Vec<String> = sentence
            .bunsetsu
            .iter()
            .map(|chunk| {
                let pieces: Vec<&str> = tokens[chunk.token_start..=chunk.token_end]
                    .iter()
                    .map(|token| token.surface.as_str())
                    .collect();
                format!("[{}]", pieces.join("·"))
            })
            .collect();
        println!("文{}: {}", index + 1, rendered.join(" "));
    }
}

/// Print the `AnalysisTree` as an indented outline.
///
/// This is the debugging view for node *shape*: it shows exactly which tokens
/// hang under which grammar/segment node, which is what the web UI renders.
/// A content word sitting in its own `segment` node next to a particle node
/// means a rule matched bare and stranded its host.
pub fn print_tree(document: &AnalysisDocument) {
    println!("input: {}", document.input);
    println!(
        "tokens: {}",
        document
            .tokens
            .iter()
            .map(|token| format!("{}:{}", token.position, token.surface))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("primary matches:");
    if document.primary_matches.is_empty() {
        println!("  (none)");
    }
    for matched in &document.primary_matches {
        println!(
            "  [{}..={}] {} — {}",
            matched.token_start, matched.token_end, matched.rule_name, matched.id
        );
    }
    println!();
    print_tree_node(document, &document.tree.root_id, 0);
}

fn print_tree_node(document: &AnalysisDocument, id: &str, depth: usize) {
    let Some(node) = document.tree.node(id) else {
        println!("{}{} <missing node>", "  ".repeat(depth), id);
        return;
    };
    println!(
        "{}{} {} {}{}{}",
        "  ".repeat(depth),
        kind_label(node.kind),
        span_label(node),
        surface_of(document, node),
        attached_matches_label(document, node),
        secondary_label(node),
    );
    for child in document.tree.children_of(id) {
        print_tree_node(document, child, depth + 1);
    }
}

fn kind_label(kind: TreeNodeKind) -> &'static str {
    match kind {
        TreeNodeKind::Document => "document",
        TreeNodeKind::Sentence => "sentence",
        TreeNodeKind::Bunsetsu => "bunsetsu",
        TreeNodeKind::Token => "token   ",
    }
}

/// Names of the primary matches attached to this node, e.g. ` ⟨Noun は～⟩`.
fn attached_matches_label(document: &AnalysisDocument, node: &TreeNode) -> String {
    let names: Vec<&str> = node
        .match_ids
        .iter()
        .filter_map(|id| {
            document
                .primary_matches
                .iter()
                .find(|matched| &matched.id == id)
                .map(|matched| matched.rule_name.as_str())
        })
        .collect();
    if names.is_empty() {
        return String::new();
    }
    format!("  ⟨{}⟩", names.join(", "))
}

fn span_label(node: &TreeNode) -> String {
    match (node.token_start, node.token_end) {
        (Some(start), Some(end)) => format!("[{start}..={end}]"),
        _ => "[--]".to_string(),
    }
}

/// Join the surfaces of every token in the node's span, so a node's own label
/// shows the text it claims rather than only its id.
fn surface_of(document: &AnalysisDocument, node: &TreeNode) -> String {
    let (Some(start), Some(end)) = (node.token_start, node.token_end) else {
        return String::new();
    };
    document
        .tokens
        .iter()
        .filter(|token| token.position >= start && token.position <= end)
        .map(|token| token.surface.as_str())
        .collect()
}

fn secondary_label(node: &TreeNode) -> String {
    if node.secondary_match_ids.is_empty() {
        return String::new();
    }
    format!("  (+{} secondary)", node.secondary_match_ids.len())
}

/// Return a fill color for a token node based on its POS category.
fn pos_color(pos1: &str) -> &'static str {
    match pos1 {
        "名詞" => "#D6EAF8",   // noun        — blue
        "動詞" => "#D5F5E3",   // verb        — green
        "助詞" => "#FDEBD0",   // particle    — orange
        "助動詞" => "#FEF9E7", // auxiliary   — yellow
        "形容詞" => "#E8DAEF", // i-adjective — purple
        "副詞" => "#FDEDEC",   // adverb      — pink
        "代名詞" => "#D6EAF8", // pronoun     — blue (same as noun)
        _ => "#F2F3F4",        // other       — grey
    }
}

/// Escape a string for use inside a DOT label (double-quotes and backslashes).
fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Emit a Graphviz DOT tree from tokens and pattern matches.
///
/// Layout:
///   - Root node = full sentence text
///   - Tokens that belong to a pattern match → grouped under that pattern node
///   - Tokens not in any match → direct children of root
///   - Order follows sentence position
///
/// Open the output in Graphviz, e.g.:
///   nnj-grammar --output dot "東京しか行かない" | dot -Tsvg -o out.svg && open out.svg
pub fn print_dot(tokens: &[Token], matches: &[PatternMatch]) {
    let font = "Hiragino Sans,Arial Unicode MS,sans-serif";

    println!("digraph {{");
    println!("  rankdir=LR");
    println!("  node [fontname=\"{}\" fontsize=12]", font);
    println!("  edge [fontname=\"{}\" fontsize=10]", font);
    println!();

    // ── Root: full sentence ───────────────────────────────────────────────────
    let sentence: String = tokens.iter().map(|t| t.surface.as_str()).collect();
    println!(
        "  root [shape=box style=filled fillcolor=\"#2C3E50\" fontcolor=white \
         fontsize=14 label=\"{}\"]",
        dot_escape(&sentence)
    );
    println!();

    // ── Figure out which tokens are claimed by a pattern ─────────────────────
    // A token can appear in multiple overlapping patterns; we use the first
    // match that claims it (sorted by token_start, which match_all guarantees).
    let mut token_pattern: Vec<Option<usize>> = vec![None; tokens.len()];
    for (pi, m) in matches.iter().enumerate() {
        for owner in token_pattern
            .iter_mut()
            .take(m.token_end + 1)
            .skip(m.token_start)
        {
            if owner.is_none() {
                *owner = Some(pi);
            }
        }
    }

    // ── Pattern nodes ─────────────────────────────────────────────────────────
    for (i, m) in matches.iter().enumerate() {
        let label = format!(
            "{}\\n[{}]\\n{}",
            dot_escape(&m.rule_name),
            dot_escape(&m.jlpt),
            dot_escape(&m.meaning_en),
        );
        println!(
            "  p{} [shape=box style=\"filled,rounded\" fillcolor=\"#FFF3CD\" label=\"{}\"]",
            i, label
        );
        println!("  root -> p{}", i);
    }
    println!();

    // ── Token nodes ───────────────────────────────────────────────────────────
    for t in tokens {
        let reading_line = if t.reading != t.surface && !t.reading.is_empty() {
            format!("\\n{}", dot_escape(&t.reading))
        } else {
            String::new()
        };
        let pos_line = if t.pos2.is_empty() {
            dot_escape(&t.pos1)
        } else {
            format!("{}・{}", dot_escape(&t.pos1), dot_escape(&t.pos2))
        };
        let conj_line = if t.conj_form.is_empty() {
            String::new()
        } else {
            format!("\\n{}", dot_escape(&t.conj_form))
        };

        let label = format!(
            "{}{}\\n{}{}",
            dot_escape(&t.surface),
            reading_line,
            pos_line,
            conj_line,
        );
        let color = pos_color(&t.pos1);
        println!(
            "  t{} [shape=box style=filled fillcolor=\"{}\" label=\"{}\"]",
            t.position, color, label
        );

        // Connect to its pattern parent, or directly to root if unclaimed
        match token_pattern[t.position] {
            Some(pi) => println!("  p{} -> t{}", pi, t.position),
            None => println!("  root -> t{}", t.position),
        }
    }

    println!("}}");
}

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
