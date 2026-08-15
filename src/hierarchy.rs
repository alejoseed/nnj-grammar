//! Build the structural analysis tree: document → sentence → bunsetsu → token.
//!
//! Structure comes from `crate::chunker` (deterministic, POS-only). Grammar
//! matches never define nodes; each match is attached to the smallest existing
//! node that fully covers its token span (bunsetsu, else sentence, else
//! document). A wrong or missing grammar match can therefore mislabel a node,
//! but can never fragment the graph.

use crate::analysis::{AnalysisTree, TreeEdge, TreeNode, TreeNodeKind};
use crate::chunker;
use crate::ranking::RankedMatches;
use crate::tokenizer::Token;

pub fn build_tree(
    tokens: &[Token],
    ranked: &RankedMatches,
    words: &[(usize, usize)],
) -> AnalysisTree {
    let root_id = "document-0".to_string();
    let root_span = (!tokens.is_empty()).then(|| (0, tokens.len() - 1));
    let mut tree = AnalysisTree {
        root_id: root_id.clone(),
        nodes: vec![node(
            root_id.clone(),
            TreeNodeKind::Document,
            root_span,
            None,
        )],
        edges: Vec::new(),
    };

    for (sentence_index, sentence) in chunker::chunk(tokens).iter().enumerate() {
        let sentence_id = format!("sentence-{sentence_index}");
        tree.nodes.push(node(
            sentence_id.clone(),
            TreeNodeKind::Sentence,
            Some((sentence.token_start, sentence.token_end)),
            None,
        ));
        tree.edges.push(TreeEdge {
            parent_id: root_id.clone(),
            child_id: sentence_id.clone(),
            order: sentence_index,
        });

        for (bunsetsu_index, bunsetsu) in sentence.bunsetsu.iter().enumerate() {
            let bunsetsu_id = format!("bunsetsu-{sentence_index}-{bunsetsu_index}");
            tree.nodes.push(node(
                bunsetsu_id.clone(),
                TreeNodeKind::Bunsetsu,
                Some((bunsetsu.token_start, bunsetsu.token_end)),
                None,
            ));
            tree.edges.push(TreeEdge {
                parent_id: sentence_id.clone(),
                child_id: bunsetsu_id.clone(),
                order: bunsetsu_index,
            });

            // Dictionary words fuse their short-unit tokens into one leaf;
            // compound spans never cross a bunsetsu boundary by construction.
            let mut order = 0;
            let mut position = bunsetsu.token_start;
            while position <= bunsetsu.token_end {
                let word = words.iter().find(|(start, _)| *start == position);
                let (child_id, kind, span, token_id) = match word {
                    Some(&(start, end)) => (
                        format!("word-{start}-{end}"),
                        TreeNodeKind::Word,
                        (start, end),
                        None,
                    ),
                    None => {
                        let token_id = format!("token-{position}");
                        (
                            token_id.clone(),
                            TreeNodeKind::Token,
                            (position, position),
                            Some(token_id),
                        )
                    }
                };
                tree.nodes.push(node(child_id.clone(), kind, Some(span), token_id));
                tree.edges.push(TreeEdge {
                    parent_id: bunsetsu_id.clone(),
                    child_id,
                    order,
                });
                position = span.1 + 1;
                order += 1;
            }
        }
    }

    attach_matches(&mut tree, ranked);
    tree
}

fn node(
    id: String,
    kind: TreeNodeKind,
    span: Option<(usize, usize)>,
    token_id: Option<String>,
) -> TreeNode {
    TreeNode {
        id,
        kind,
        token_start: span.map(|span| span.0),
        token_end: span.map(|span| span.1),
        token_id,
        match_ids: Vec::new(),
        secondary_match_ids: Vec::new(),
    }
}

/// Attach every match to the smallest non-token node that fully covers its
/// span. Tokens are excluded so a single-particle match labels its bunsetsu
/// (the は match labels [わたし+は]) rather than disappearing onto a leaf.
fn attach_matches(tree: &mut AnalysisTree, ranked: &RankedMatches) {
    let primary: Vec<(String, usize, usize)> = ranked
        .primary
        .iter()
        .map(|matched| (matched.id.clone(), matched.token_start, matched.token_end))
        .collect();
    for (id, start, end) in primary {
        if let Some(owner) = smallest_cover(tree, start, end) {
            tree.nodes[owner].match_ids.push(id);
        }
    }

    let secondary: Vec<(String, usize, usize)> = ranked
        .secondary
        .iter()
        .map(|secondary| {
            (
                secondary.id.clone(),
                secondary.matched.token_start,
                secondary.matched.token_end,
            )
        })
        .collect();
    for (id, start, end) in secondary {
        if let Some(owner) = smallest_cover(tree, start, end) {
            tree.nodes[owner].secondary_match_ids.push(id);
        }
    }
}

fn smallest_cover(tree: &AnalysisTree, token_start: usize, token_end: usize) -> Option<usize> {
    tree.nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            !matches!(node.kind, TreeNodeKind::Token | TreeNodeKind::Word)
                && node
                    .token_start
                    .zip(node.token_end)
                    .is_some_and(|span| span.0 <= token_start && span.1 >= token_end)
        })
        .min_by_key(|(_, node)| {
            let span = node
                .token_start
                .zip(node.token_end)
                .map_or(usize::MAX, |span| span.1 - span.0);
            // On equal spans (single-sentence input), prefer the deeper node.
            let depth = match node.kind {
                TreeNodeKind::Bunsetsu => 0,
                TreeNodeKind::Sentence => 1,
                _ => 2,
            };
            (span, depth)
        })
        .map(|(index, _)| index)
}
