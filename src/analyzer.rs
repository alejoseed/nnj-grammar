use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::analysis::{AnalysisDocument, AnalyzedToken, ANALYSIS_SCHEMA_VERSION};
use crate::dictionary::Dictionary;
use crate::hierarchy::build_tree;
use crate::matcher::match_candidates;
use crate::patterns::{load_combined, PatternRule};
use crate::ranking::rank_candidates;
use crate::tokenizer::Tokenizer;

#[derive(Debug, Clone, Default)]
pub struct AnalyzerConfig {
    pub local_grammar_dir: Option<PathBuf>,
    pub dictionary_path: Option<PathBuf>,
}

pub struct Analyzer {
    tokenizer: Tokenizer,
    rules: Vec<PatternRule>,
}

impl Analyzer {
    pub fn new(config: AnalyzerConfig) -> Result<Self> {
        anyhow::ensure!(
            config.dictionary_path.is_none(),
            "dictionary support is not implemented yet; omit dictionary_path"
        );
        if let Some(path) = config.local_grammar_dir.as_deref() {
            anyhow::ensure!(
                path.is_dir(),
                "local grammar directory does not exist or is not a directory: {}",
                path.display()
            );
        }
        let tokenizer = Tokenizer::new().context("failed to initialize embedded UniDic")?;
        let rules = load_combined(config.local_grammar_dir.as_deref())
            .context("failed to load combined grammar catalog")?;
        Ok(Self { tokenizer, rules })
    }

    pub fn analyze(&self, text: &str) -> Result<AnalysisDocument> {
        let tokens = self
            .tokenizer
            .tokenize(text)
            .context("failed to tokenize input")?;
        let ranked = rank_candidates(match_candidates(&tokens, &self.rules));
        let token_glosses = Dictionary::shared().gloss_tokens(&tokens);
        let analyzed_tokens = tokens
            .iter()
            .enumerate()
            .map(|(index, token)| {
                let mut analyzed = AnalyzedToken::from(token);
                analyzed.glosses = token_glosses[index].clone();
                analyzed
            })
            .collect();
        let tree = build_tree(&tokens, &ranked);

        Ok(AnalysisDocument {
            schema_version: ANALYSIS_SCHEMA_VERSION,
            input: text.to_string(),
            tokens: analyzed_tokens,
            primary_matches: ranked.primary,
            secondary_matches: ranked.secondary,
            tree,
        })
    }
}
