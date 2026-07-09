mod cli;
mod display;
mod matcher;
mod patterns;
mod tokenizer;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, OutputFormat};

fn main() -> Result<()> {
    let cli = Cli::parse();

    let text = match cli.read_text()? {
        Some(t) => t,
        None => {
            // No input provided — print help and exit cleanly
            <Cli as clap::CommandFactory>::command().print_help()?;
            println!();
            return Ok(());
        }
    };

    let tokenizer = tokenizer::Tokenizer::new()?;
    let tokens = tokenizer.tokenize(&text)?;

    match cli.output {
        OutputFormat::Table => {
            tokenizer::print_table(&tokens);
        }
        OutputFormat::Raw => {
            tokenizer::print_raw(&tokenizer, &text)?;
        }
        OutputFormat::Graph => {
            let rules = patterns::load_grammar_dir(&cli.grammar_db)?;
            let matches = matcher::match_all(&tokens, &rules);
            display::print_graph(&tokens, &matches);
        }
        OutputFormat::Json => {
            let rules = patterns::load_grammar_dir(&cli.grammar_db)?;
            let matches = matcher::match_all(&tokens, &rules);

            #[derive(serde::Serialize)]
            struct Output<'a> {
                input: &'a str,
                tokens: &'a [tokenizer::Token],
                matches: &'a [matcher::PatternMatch],
            }

            let output = Output { input: &text, tokens: &tokens, matches: &matches };
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Dot => {
            let rules = patterns::load_grammar_dir(&cli.grammar_db)?;
            let matches = matcher::match_all(&tokens, &rules);
            display::print_dot(&tokens, &matches);
        }
    }

    Ok(())
}
