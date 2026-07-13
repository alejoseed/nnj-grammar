use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;

use crate::matcher;
use crate::patterns::{self, PatternRule};
use crate::tokenizer::{Token, Tokenizer};

const REGRESSION_JSON: &str = include_str!("../grammar/hanabira/regression.json");

static TOKENIZER: OnceLock<Tokenizer> = OnceLock::new();
static RULES: OnceLock<Vec<PatternRule>> = OnceLock::new();
static RULE_IDS: OnceLock<HashMap<String, usize>> = OnceLock::new();

#[derive(Deserialize)]
struct RegressionManifest {
    examples: Vec<RegressionExample>,
    logical_rules: usize,
}

#[derive(Deserialize)]
struct RegressionExample {
    jp: String,
    owning_rule_id: String,
}

fn tokenizer() -> &'static Tokenizer {
    TOKENIZER.get_or_init(|| Tokenizer::new().expect("embedded UniDic should initialize"))
}

fn rules() -> &'static [PatternRule] {
    RULES.get_or_init(|| patterns::load_embedded().expect("embedded Hanabira rules should load"))
}

fn rule_ids() -> &'static HashMap<String, usize> {
    RULE_IDS.get_or_init(|| {
        rules()
            .iter()
            .enumerate()
            .map(|(index, rule)| (rule.id.clone(), index))
            .collect()
    })
}

#[test]
fn owning_examples_meet_hanabira_regression_baseline() {
    const MIN_OWNING_EXAMPLE_RECALL: f64 = 0.66;
    const MIN_RULE_COVERAGE: f64 = 0.77;

    let manifest: RegressionManifest =
        serde_json::from_str(REGRESSION_JSON).expect("regression.json should be valid");
    assert_eq!(manifest.logical_rules, rules().len());

    let mut tokenized: HashMap<&str, Vec<Token>> = HashMap::new();
    for example in &manifest.examples {
        tokenized.entry(&example.jp).or_insert_with(|| {
            tokenizer()
                .tokenize(&example.jp)
                .unwrap_or_else(|error| panic!("failed to tokenize {:?}: {error}", example.jp))
        });
    }

    let mut recalled_examples = 0;
    let mut covered_rules = HashSet::new();
    for example in &manifest.examples {
        let rule_index = *rule_ids()
            .get(&example.owning_rule_id)
            .unwrap_or_else(|| panic!("missing owning rule {}", example.owning_rule_id));
        let owner = &rules()[rule_index];
        let matches =
            matcher::match_all(&tokenized[example.jp.as_str()], std::slice::from_ref(owner));
        if !matches.is_empty() {
            recalled_examples += 1;
            covered_rules.insert(owner.id.as_str());
        }
    }

    let recall = recalled_examples as f64 / manifest.examples.len() as f64;
    let coverage = covered_rules.len() as f64 / manifest.logical_rules as f64;
    println!(
        "Hanabira regression: examples={}/{} ({:.2}%), rules={}/{} ({:.2}%), unique_sources={}",
        recalled_examples,
        manifest.examples.len(),
        recall * 100.0,
        covered_rules.len(),
        manifest.logical_rules,
        coverage * 100.0,
        tokenized.len()
    );

    assert!(
        recall >= MIN_OWNING_EXAMPLE_RECALL,
        "owning-example recall {:.2}% is below {:.2}%",
        recall * 100.0,
        MIN_OWNING_EXAMPLE_RECALL * 100.0
    );
    assert!(
        coverage >= MIN_RULE_COVERAGE,
        "rule coverage {:.2}% is below {:.2}%",
        coverage * 100.0,
        MIN_RULE_COVERAGE * 100.0
    );
}
