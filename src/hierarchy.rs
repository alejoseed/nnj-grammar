use crate::analysis::{AnalysisTree, TreeEdge, TreeNode, TreeNodeKind};
use crate::ranking::RankedMatches;
use crate::tokenizer::Token;

pub fn build_tree(tokens: &[Token], ranked: &RankedMatches) -> AnalysisTree {
    let root_id = "sentence-0".to_string();
    let root_span = (!tokens.is_empty()).then(|| (0, tokens.len() - 1));
    let mut tree = AnalysisTree {
        root_id: root_id.clone(),
        nodes: vec![TreeNode {
            id: root_id.clone(),
            kind: TreeNodeKind::Sentence,
            token_start: root_span.map(|span| span.0),
            token_end: root_span.map(|span| span.1),
            token_id: None,
            match_id: None,
            secondary_match_ids: Vec::new(),
        }],
        edges: Vec::new(),
    };

    let mut cursor = 0;
    for matched in &ranked.primary {
        if cursor < matched.token_start {
            add_span(
                &mut tree,
                &root_id,
                TreeNodeKind::Segment,
                format!("segment-{cursor}-{}", matched.token_start - 1),
                cursor,
                matched.token_start - 1,
                None,
            );
        }
        add_span(
            &mut tree,
            &root_id,
            TreeNodeKind::Grammar,
            matched.id.clone(),
            matched.token_start,
            matched.token_end,
            Some(matched.id.clone()),
        );
        cursor = matched.token_end + 1;
    }
    if cursor < tokens.len() {
        add_span(
            &mut tree,
            &root_id,
            TreeNodeKind::Segment,
            format!("segment-{cursor}-{}", tokens.len() - 1),
            cursor,
            tokens.len() - 1,
            None,
        );
    }

    attach_secondary_matches(&mut tree, ranked);
    tree
}

fn add_span(
    tree: &mut AnalysisTree,
    root_id: &str,
    kind: TreeNodeKind,
    id: String,
    token_start: usize,
    token_end: usize,
    match_id: Option<String>,
) {
    let root_order = tree.children_of(root_id).len();
    tree.edges.push(TreeEdge {
        parent_id: root_id.to_string(),
        child_id: id.clone(),
        order: root_order,
    });
    tree.nodes.push(TreeNode {
        id: id.clone(),
        kind,
        token_start: Some(token_start),
        token_end: Some(token_end),
        token_id: None,
        match_id,
        secondary_match_ids: Vec::new(),
    });

    for (order, position) in (token_start..=token_end).enumerate() {
        let token_id = format!("token-{position}");
        tree.edges.push(TreeEdge {
            parent_id: id.clone(),
            child_id: token_id.clone(),
            order,
        });
        tree.nodes.push(TreeNode {
            id: token_id.clone(),
            kind: TreeNodeKind::Token,
            token_start: Some(position),
            token_end: Some(position),
            token_id: Some(token_id),
            match_id: None,
            secondary_match_ids: Vec::new(),
        });
    }
}

fn attach_secondary_matches(tree: &mut AnalysisTree, ranked: &RankedMatches) {
    for secondary in &ranked.secondary {
        let token_start = secondary.matched.token_start;
        let token_end = secondary.matched.token_end;
        let owner = tree
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                matches!(node.kind, TreeNodeKind::Grammar | TreeNodeKind::Segment)
                    && node
                        .token_start
                        .zip(node.token_end)
                        .is_some_and(|span| span.0 <= token_start && span.1 >= token_end)
            })
            .min_by_key(|(_, node)| {
                node.token_start
                    .zip(node.token_end)
                    .map_or(usize::MAX, |span| span.1 - span.0)
            })
            .map(|(index, _)| index)
            .unwrap_or(0);
        tree.nodes[owner]
            .secondary_match_ids
            .push(secondary.id.clone());
    }
}
