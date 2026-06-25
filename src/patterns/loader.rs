use anyhow::{Context, Result};
use std::path::Path;
use walkdir::WalkDir;

use super::rule::{GrammarFile, PatternRule};

/// Walk `dir` recursively and load every `*.toml` file as a grammar rule file.
///
/// Files that fail to parse emit a warning to stderr and are skipped —
/// one bad file should not block the rest from loading.
///
/// Returns an error only if `dir` does not exist or cannot be read at all.
pub fn load_grammar_dir(dir: &Path) -> Result<Vec<PatternRule>> {
    anyhow::ensure!(
        dir.exists(),
        "grammar DB directory not found: {}",
        dir.display()
    );

    let mut rules: Vec<PatternRule> = Vec::new();

    for entry in WalkDir::new(dir).follow_links(true) {
        let entry = entry.context("error walking grammar directory")?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        match load_file(path) {
            Ok(mut file_rules) => rules.append(&mut file_rules),
            Err(e) => eprintln!("warning: skipping {:?}: {}", path, e),
        }
    }

    Ok(rules)
}

fn load_file(path: &Path) -> Result<Vec<PatternRule>> {
    let src = std::fs::read_to_string(path)
        .with_context(|| format!("could not read {}", path.display()))?;

    let file: GrammarFile = toml::from_str(&src)
        .with_context(|| format!("invalid TOML in {}", path.display()))?;

    Ok(file.patterns)
}
