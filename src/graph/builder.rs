use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};

use crate::matcher::PatternMatch;
use crate::tokenizer::Token;

// ── Node types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenNode {
    pub id: usize,
    pub surface: String,
    pub pos1: String,
    pub pos2: String,
    pub pos3: String,
    pub pos4: String,
    pub conj_type: String,
    pub conj_form: String,
    pub base_form: String,
    pub reading: String,
    pub byte_start: usize,
    pub byte_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternNode {
    pub id: usize,
    pub name: String,
    pub jlpt: String,
    pub meaning_en: String,
    pub hint: Option<String>,
    pub token_start: usize,
    pub token_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeKind {
    Token(TokenNode),
    Pattern(PatternNode),
}

// ── Edge types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// token[i] → token[i+1]: the linear sentence spine
    Sequence,
    /// first token of match → pattern node
    PatternSpan,
    /// last token of match → pattern node
    PatternEnd,
}

// ── Builder ──────────────────────────────────────────────────────────────────

/// Build a directed graph from a token stream and its pattern matches.
///
/// ## Your job
///
/// 1. Add one `NodeKind::Token` per token in order.
///    Keep a `Vec<NodeIndex>` indexed by token position so you can reference
///    them when adding edges.
///
/// 2. Add `EdgeKind::Sequence` from node_indices[i] → node_indices[i+1]
///    for each consecutive pair of tokens.
///
/// 3. For each `PatternMatch`:
///    - Add one `NodeKind::Pattern` node.
///    - Add `EdgeKind::PatternSpan` from node_indices[m.token_start] → pattern.
///    - Add `EdgeKind::PatternEnd`  from node_indices[m.token_end]   → pattern.
pub fn build_graph(tokens: &[Token], matches: &[PatternMatch]) -> DiGraph<NodeKind, EdgeKind> {
    let mut graph: DiGraph<NodeKind, EdgeKind> = DiGraph::new();

    // TODO: implement graph construction
    // Hint: petgraph API you'll use —
    //   let idx = graph.add_node(NodeKind::Token(...));
    //   graph.add_edge(idx_a, idx_b, EdgeKind::Sequence);

    let _ = (tokens, matches); // suppress unused warnings until implemented
    graph
}
