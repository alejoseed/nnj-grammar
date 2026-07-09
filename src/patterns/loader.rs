use anyhow::{Context, Result};
use rust_embed::RustEmbed;
use std::path::Path;
use walkdir::WalkDir;

use super::rule::{GrammarFile, PatternRule};

/// All TOML files under grammar/ are compiled into the binary.
/// This means the binary is self-contained — no grammar directory needed at runtime.
/// To update rules: edit the TOML files and recompile (normal app update workflow).
#[derive(RustEmbed)]
#[folder = "grammar/"]
#[include = "**/*.toml"]
struct EmbeddedGrammar;

/// Load grammar rules from the embedded files baked into the binary.
/// Use this on iOS or when distributing a standalone binary.
pub fn load_embedded() -> Result<Vec<PatternRule>> {
    let mut rules = Vec::new();

    for filename in EmbeddedGrammar::iter() {
        let file = EmbeddedGrammar::get(&filename)
            .with_context(|| format!("failed to read embedded file: {}", filename))?;

        let src = std::str::from_utf8(file.data.as_ref())
            .with_context(|| format!("embedded file is not valid UTF-8: {}", filename))?;

        match parse_toml(src, &filename) {
            Ok(mut file_rules) => rules.append(&mut file_rules),
            Err(e) => eprintln!("warning: skipping embedded {}: {}", filename, e),
        }
    }

    Ok(rules)
}

/// Load grammar rules from a directory on the filesystem.
/// Used when --grammar-db is passed explicitly (development, custom rule sets).
pub fn load_grammar_dir(dir: &Path) -> Result<Vec<PatternRule>> {
    anyhow::ensure!(
        dir.exists(),
        "grammar directory not found: {}",
        dir.display()
    );

    let mut rules = Vec::new();

    for entry in WalkDir::new(dir).follow_links(true) {
        let entry = entry.context("error walking grammar directory")?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let src = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;

        match parse_toml(&src, &path.display().to_string()) {
            Ok(mut file_rules) => rules.append(&mut file_rules),
            Err(e) => eprintln!("warning: skipping {}: {}", path.display(), e),
        }
    }

    Ok(rules)
}

fn parse_toml(src: &str, name: &str) -> Result<Vec<PatternRule>> {
    let file: GrammarFile = toml::from_str(src)
        .with_context(|| format!("invalid TOML in {}", name))?;
    Ok(file.patterns)
}
