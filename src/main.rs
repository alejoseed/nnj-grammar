mod cli;
mod display;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, OutputFormat};
use nnj_grammar::{matcher, patterns, tokenizer};

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
            return Ok(());
        }
        OutputFormat::Raw => {
            tokenizer::print_raw(&tokenizer, &text)?;
            return Ok(());
        }
        OutputFormat::Tokens => {
            println!("{}", serde_json::to_string(&tokens)?);
            return Ok(());
        }

        output_fmt => {
            // Load rules: embedded by default, filesystem if --grammar-db was set
            let default_db = std::path::PathBuf::from("grammar");
            let rules = if cli.grammar_db != default_db {
                patterns::load_grammar_dir(&cli.grammar_db)?
            } else {
                patterns::load_embedded()?
            };

            let matches = matcher::match_all(&tokens, &rules);

            match output_fmt {
                OutputFormat::Graph => display::print_graph(&tokens, &matches),
                OutputFormat::Dot => display::print_dot(&tokens, &matches),
                OutputFormat::Json => {
                    #[derive(serde::Serialize)]
                    struct Output<'a> {
                        input: &'a str,
                        tokens: &'a [tokenizer::Token],
                        matches: &'a [matcher::PatternMatch],
                    }
                    let output = Output {
                        input: &text,
                        tokens: &tokens,
                        matches: &matches,
                    };
                    println!("{}", serde_json::to_string_pretty(&output)?);
                }
                OutputFormat::Table | OutputFormat::Raw | OutputFormat::Tokens => unreachable!(),
            }
        }
    }

    Ok(())
}
