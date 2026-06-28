mod cli;
mod display;
mod matcher;
mod patterns;
mod tokenizer;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, OutputFormat};

#[derive(serde::Serialize)]
struct Output<'a> {
    input: &'a str,
    tokens: &'a [tokenizer::Token],
    matches: &'a [matcher::PatternMatch],
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let text = cli.read_text()?;

    // Step 1: tokenize
    let tokenizer = tokenizer::Tokenizer::new()?;
    let tokens = tokenizer.tokenize(&text)?;

    if matches!(cli.output, OutputFormat::Table) {
        tokenizer::print_table(&tokens);
        return Ok(());
    }

    if matches!(cli.output, OutputFormat::Raw) {
        tokenizer::print_raw(&tokenizer, &text)?;
        return Ok(());
    }

    if matches!(cli.output, OutputFormat::Graph) {
        let rules = patterns::load_grammar_dir(&cli.grammar_db)?;
        let pattern_matches = matcher::match_all(&tokens, &rules);
        display::print_graph(&tokens, &pattern_matches);
        return Ok(());
    }

    // Step 2: load grammar rules
    let rules = patterns::load_grammar_dir(&cli.grammar_db)?;

    // Step 3: match patterns
    let matches = matcher::match_all(&tokens, &rules);

    // Step 4: serialize
    let output = Output {
        input: &text,
        tokens: &tokens,
        matches: &matches,
    };

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}
