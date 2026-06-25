mod cli;
mod graph;
mod matcher;
mod patterns;
mod tokenizer;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, OutputFormat};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let text = cli.read_text()?;

    // ── Step 1: tokenize ─────────────────────────────────────────────────────
    let tokenizer = tokenizer::Tokenizer::new()?;
    let tokens = tokenizer.tokenize(&text)?;

    // Table mode: just show what the tokenizer produces — useful while you're
    // exploring and building the matcher.
    if matches!(cli.output, OutputFormat::Table) {
        tokenizer::print_table(&tokens);
        return Ok(());
    }

    // ── Step 2: load grammar rules ───────────────────────────────────────────
    let rules = patterns::load_grammar_dir(&cli.grammar_db)?;

    // ── Step 3: match patterns ───────────────────────────────────────────────
    let matches = matcher::match_all(&tokens, &rules);

    // ── Step 4: build graph ──────────────────────────────────────────────────
    let g = graph::build_graph(&tokens, &matches);

    // ── Step 5: serialize and print ──────────────────────────────────────────
    match cli.output {
        OutputFormat::Json => {
            let v = graph::to_json(&g, &text);
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        OutputFormat::Dot => {
            println!("{}", graph::to_dot(&g));
        }
        OutputFormat::Table => unreachable!(),
    }

    Ok(())
}
