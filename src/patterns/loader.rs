use anyhow::{Context, Result};
use rust_embed::RustEmbed;
use std::path::Path;
use walkdir::WalkDir;

use super::rule::{CatalogSource, GrammarFile, PatternRule, Step};

/// Generated Hanabira TOML files are compiled into the binary.
/// This means the binary is self-contained — no grammar directory needed at runtime.
/// To update rules: edit the TOML files and recompile (normal app update workflow).
#[derive(RustEmbed)]
#[folder = "grammar/hanabira/"]
#[include = "*.toml"]
struct EmbeddedGrammar;

/// Load grammar rules from the embedded files baked into the binary.
/// Use this on iOS or when distributing a standalone binary.
pub fn load_embedded() -> Result<Vec<PatternRule>> {
    let mut rules = Vec::new();
    let mut filenames: Vec<_> = EmbeddedGrammar::iter().collect();
    filenames.sort();

    for filename in filenames {
        let file = EmbeddedGrammar::get(&filename)
            .with_context(|| format!("failed to read embedded file: {}", filename))?;

        let src = std::str::from_utf8(file.data.as_ref())
            .with_context(|| format!("embedded file is not valid UTF-8: {}", filename))?;

        let mut file_rules = parse_toml_with_source(
            src,
            &filename,
            &CatalogSource::new("hanabira", "Hanabira"),
        )?;
        rules.append(&mut file_rules);
    }

    validate_unique_rule_ids(&rules, "embedded grammar catalog")?;
    Ok(rules)
}

/// Load grammar rules from a directory on the filesystem.
/// Used when --grammar-db is passed explicitly (development, custom rule sets).
pub fn load_grammar_dir(dir: &Path) -> Result<Vec<PatternRule>> {
    load_grammar_dir_with_source(dir, &CatalogSource::new("filesystem", "Filesystem"))
}

fn load_grammar_dir_with_source(
    dir: &Path,
    source: &CatalogSource,
) -> Result<Vec<PatternRule>> {
    anyhow::ensure!(
        dir.exists(),
        "grammar directory not found: {}",
        dir.display()
    );

    let mut rules = Vec::new();

    for entry in WalkDir::new(dir).follow_links(true).sort_by_file_name() {
        let entry = entry.context("error walking grammar directory")?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let src = std::fs::read_to_string(path)
            .with_context(|| format!("could not read {}", path.display()))?;

        let mut file_rules = parse_toml_with_source(&src, &path.display().to_string(), source)?;
        rules.append(&mut file_rules);
    }

    validate_unique_rule_ids(&rules, &dir.display().to_string())?;
    Ok(rules)
}

/// Load embedded Hanabira together with an optional personal local catalog.
pub fn load_combined(local_dir: Option<&Path>) -> Result<Vec<PatternRule>> {
    let mut rules = load_embedded()?;
    if let Some(dir) = local_dir.filter(|dir| dir.exists()) {
        rules.extend(load_grammar_dir_with_source(
            dir,
            &CatalogSource::new("bunpro-local", "Bunpro local"),
        )?);
    }
    validate_unique_rule_ids(&rules, "combined grammar catalog")?;
    Ok(rules)
}

fn validate_unique_rule_ids(rules: &[PatternRule], name: &str) -> Result<()> {
    let mut ids = std::collections::HashSet::new();
    for rule in rules {
        anyhow::ensure!(
            ids.insert(rule.id.as_str()),
            "duplicate pattern id {} in {}",
            rule.id,
            name
        );
    }
    Ok(())
}

#[cfg(test)]
fn parse_toml(src: &str, name: &str) -> Result<Vec<PatternRule>> {
    parse_toml_with_source(src, name, &CatalogSource::default())
}

fn parse_toml_with_source(
    src: &str,
    name: &str,
    source: &CatalogSource,
) -> Result<Vec<PatternRule>> {
    let file: GrammarFile =
        toml::from_str(src).with_context(|| format!("invalid TOML in {}", name))?;
    let mut rules = file.patterns;
    validate_rules(&rules, name)?;
    for rule in &mut rules {
        rule.source = source.clone();
    }
    Ok(rules)
}

fn validate_rules(rules: &[PatternRule], name: &str) -> Result<()> {
    for rule in rules {
        anyhow::ensure!(!rule.id.is_empty(), "empty pattern id in {}", name);
        anyhow::ensure!(
            !rule.steps.is_empty() || !rule.variants.is_empty(),
            "pattern {} has no steps or variants in {}",
            rule.id,
            name
        );
        validate_steps(&rule.steps, &rule.id, name)?;

        let mut variant_ids = std::collections::HashSet::new();
        for variant in &rule.variants {
            anyhow::ensure!(
                !variant.id.is_empty(),
                "pattern {} has an empty variant id in {}",
                rule.id,
                name
            );
            anyhow::ensure!(
                variant_ids.insert(&variant.id),
                "pattern {} has duplicate variant id {} in {}",
                rule.id,
                variant.id,
                name
            );
            anyhow::ensure!(
                !variant.core.is_empty(),
                "pattern {} variant {} has no core steps in {}",
                rule.id,
                variant.id,
                name
            );
            validate_steps(&variant.core, &rule.id, name)?;
            validate_steps(&variant.left_context, &rule.id, name)?;
            validate_steps(&variant.right_context, &rule.id, name)?;
        }
    }
    Ok(())
}

fn validate_steps(steps: &[Step], rule_id: &str, name: &str) -> Result<()> {
    for step in steps {
        if let Some(wildcard) = &step.wildcard {
            anyhow::ensure!(
                wildcard.min <= wildcard.max,
                "pattern {} has wildcard min greater than max in {}",
                rule_id,
                name
            );
            anyhow::ensure!(
                step.surface.is_none()
                    && step.pos1.is_none()
                    && step.pos2.is_none()
                    && step.conj_form.is_none()
                    && step.base_form.is_none()
                    && step.one_of.is_empty(),
                "pattern {} mixes wildcard and token predicates in {}",
                rule_id,
                name
            );
        } else {
            anyhow::ensure!(
                step.surface.is_some()
                    || step.pos1.is_some()
                    || step.pos2.is_some()
                    || step.conj_form.is_some()
                    || step.base_form.is_some()
                    || !step.one_of.is_empty(),
                "pattern {} has an unconstrained token step in {}",
                rule_id,
                name
            );
        }
        anyhow::ensure!(
            step.capture.as_deref() != Some(""),
            "pattern {} has an empty capture name in {}",
            rule_id,
            name
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load_combined, load_embedded, parse_toml};
    use std::collections::HashSet;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn embedded_hanabira_catalog_is_complete_and_valid() {
        let rules = load_embedded().expect("embedded Hanabira rules should parse");
        let unique_ids: HashSet<&str> = rules.iter().map(|rule| rule.id.as_str()).collect();

        assert_eq!(rules.len(), 828);
        assert_eq!(unique_ids.len(), rules.len());
        assert!(rules
            .iter()
            .all(|rule| !rule.steps.is_empty() || !rule.variants.is_empty()));
    }

    #[test]
    fn parses_legacy_steps_and_explicit_variants() {
        let rules = parse_toml(
            r#"
                [[patterns]]
                id = "legacy"
                name = "legacy"
                jlpt = "N5"
                [[patterns.steps]]
                surface = "が"

                [[patterns]]
                id = "variants"
                name = "variants"
                jlpt = "N3"
                ambiguity_group = "ga"
                [[patterns.variants]]
                id = "contrast"
                right_boundary = "clause"
                [[patterns.variants.left_context]]
                pos1 = "動詞"
                [[patterns.variants.core]]
                surface = "が"
                one_of = ["が", { pos1 = "助詞" }]
                optional = true
                capture = "marker"
            "#,
            "test.toml",
        )
        .expect("both schemas should parse");

        assert_eq!(rules[0].steps[0].surface.as_deref(), Some("が"));
        assert_eq!(rules[1].variants[0].id, "contrast");
        assert_eq!(rules[1].variants[0].core[0].one_of.len(), 2);
    }

    #[test]
    fn rejects_invalid_wildcard_bounds() {
        let error = parse_toml(
            r#"
                [[patterns]]
                id = "bad"
                name = "bad"
                jlpt = "N5"
                [[patterns.steps]]
                wildcard = { min = 3, max = 1 }
            "#,
            "bad.toml",
        )
        .expect_err("invalid bounds must fail the file");

        assert!(error.to_string().contains("min greater than max"));
    }

    #[test]
    fn combined_catalog_adds_local_rules_with_provenance() {
        let local = tempdir().expect("temporary local catalog");
        fs::write(
            local.path().join("bunpro-local.toml"),
            r#"
                [[patterns]]
                id = "bunpro-local-test"
                name = "test"
                jlpt = "N5"
                [[patterns.steps]]
                surface = "てすと"
            "#,
        )
        .expect("write local fixture");

        let rules = load_combined(Some(local.path())).expect("combined catalog should load");

        assert_eq!(
            rules
                .iter()
                .filter(|rule| rule.source.id == "hanabira")
                .count(),
            828
        );
        assert!(rules.iter().any(|rule| {
            rule.id == "bunpro-local-test" && rule.source.id == "bunpro-local"
        }));
    }

    #[test]
    fn combined_catalog_rejects_duplicate_ids_across_sources() {
        let embedded = load_embedded().expect("embedded rules");
        let duplicate_id = &embedded[0].id;
        let local = tempdir().expect("temporary local catalog");
        fs::write(
            local.path().join("bunpro-local.toml"),
            format!(
                r#"
                    [[patterns]]
                    id = "{duplicate_id}"
                    name = "duplicate"
                    jlpt = "N5"
                    [[patterns.steps]]
                    surface = "重複"
                "#
            ),
        )
        .expect("write duplicate fixture");

        let error = load_combined(Some(local.path())).expect_err("duplicate IDs must fail");

        assert!(error.to_string().contains("duplicate pattern id"));
    }
}
