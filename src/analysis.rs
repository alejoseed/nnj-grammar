use serde::Serialize;

use crate::ranking::{DisplayMatch, SecondaryMatch};
use crate::tokenizer::Token;

pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisDocument {
    pub schema_version: u32,
    pub input: String,
    pub tokens: Vec<AnalyzedToken>,
    pub primary_matches: Vec<DisplayMatch>,
    pub secondary_matches: Vec<SecondaryMatch>,
    pub tree: AnalysisTree,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyzedToken {
    pub id: String,
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
    pub position: usize,
    pub glosses: Vec<DictionaryGloss>,
}

impl From<&Token> for AnalyzedToken {
    fn from(token: &Token) -> Self {
        Self {
            id: format!("token-{}", token.position),
            surface: token.surface.clone(),
            pos1: token.pos1.clone(),
            pos2: token.pos2.clone(),
            pos3: token.pos3.clone(),
            pos4: token.pos4.clone(),
            conj_type: token.conj_type.clone(),
            conj_form: token.conj_form.clone(),
            base_form: token.base_form.clone(),
            reading: token.reading.clone(),
            byte_start: token.byte_start,
            byte_end: token.byte_end,
            position: token.position,
            glosses: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DictionaryGloss {
    pub entry_seq: i64,
    pub gloss: String,
    pub pos: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnalysisTree {
    pub root_id: String,
    pub nodes: Vec<TreeNode>,
    pub edges: Vec<TreeEdge>,
}

impl AnalysisTree {
    pub fn node(&self, id: &str) -> Option<&TreeNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    pub fn children_of(&self, id: &str) -> Vec<&str> {
        let mut edges: Vec<_> = self
            .edges
            .iter()
            .filter(|edge| edge.parent_id == id)
            .collect();
        edges.sort_by_key(|edge| edge.order);
        edges
            .into_iter()
            .map(|edge| edge.child_id.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeNodeKind {
    Sentence,
    Grammar,
    Segment,
    Token,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TreeNode {
    pub id: String,
    pub kind: TreeNodeKind,
    pub token_start: Option<usize>,
    pub token_end: Option<usize>,
    pub token_id: Option<String>,
    pub match_id: Option<String>,
    pub secondary_match_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TreeEdge {
    pub parent_id: String,
    pub child_id: String,
    pub order: usize,
}
