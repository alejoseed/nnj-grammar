//! The web frontend generates its Zod schemas from the committed JSON Schema,
//! so a change to the analysis document must regenerate it:
//!
//!     UPDATE_SCHEMA=1 cargo test --test analysis_schema

use std::path::Path;

use nnj_grammar::analysis::AnalysisDocument;

#[test]
fn committed_schema_matches_the_analysis_document() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/analysis-document.schema.json");
    let schema = schemars::schema_for!(AnalysisDocument);
    let generated = serde_json::to_string_pretty(&schema).expect("schema serializes") + "\n";

    if std::env::var_os("UPDATE_SCHEMA").is_some() {
        std::fs::write(&path, &generated).expect("schema file is writable");
        return;
    }

    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == generated,
        "{} is stale; run `UPDATE_SCHEMA=1 cargo test --test analysis_schema`",
        path.display()
    );
}
