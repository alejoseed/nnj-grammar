mod cli;
mod display;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, OutputFormat};
use nnj_grammar::analyzer::{Analyzer, AnalyzerConfig};
use nnj_grammar::{matcher, patterns, tokenizer};

/// `--grammar-db` left at its default means "embedded catalog only", matching
/// the analyzer's `AnalyzerConfig::default()`. An explicit directory is loaded
/// as a local overlay on top of the embedded catalog, the same way
/// `nnj-grammar-server` treats `grammar/local`.
fn analyzer_config(cli: &Cli) -> AnalyzerConfig {
    let default_db = std::path::PathBuf::from("grammar");
    AnalyzerConfig {
        local_grammar_dir: (cli.grammar_db != default_db).then(|| cli.grammar_db.clone()),
        dictionary_path: None,
    }
}

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

    // The analyzer path owns its own tokenizer and produces the same
    // AnalysisDocument the server serves, so run it before the legacy path
    // instead of tokenizing twice.
    match cli.output {
        OutputFormat::Json | OutputFormat::Tree => {
            let analyzer = Analyzer::new(analyzer_config(&cli))
                .context("failed to initialize analyzer")?;
            let document = analyzer.analyze(&text).context("failed to analyze input")?;
            match cli.output {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&document)?),
                OutputFormat::Tree => display::print_tree(&document),
                _ => unreachable!(),
            }
            return Ok(());
        }
        _ => {}
    }

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
        OutputFormat::Bunsetsu => {
            let sentences = nnj_grammar::chunker::chunk(&tokens);
            display::print_bunsetsu(&tokens, &sentences);
            return Ok(());
        }
        OutputFormat::BunsetsuTrace => {
            let sentences = nnj_grammar::chunker::chunk(&tokens);
            display::print_bunsetsu(&tokens, &sentences);
            println!();
            display::print_bunsetsu_trace(&tokens, &nnj_grammar::chunker::trace(&tokens));
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
                OutputFormat::Json
                | OutputFormat::Tree
                | OutputFormat::Bunsetsu
                | OutputFormat::BunsetsuTrace
                | OutputFormat::Table
                | OutputFormat::Raw
                | OutputFormat::Tokens => unreachable!(),
            }
        }
    }

    Ok(())
}
