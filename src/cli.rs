use anyhow::Result;
use clap::Parser;
use std::io::Read;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "nnj-grammar", version, about = "Japanese grammar pattern graph builder")]
pub struct Cli {
    /// Japanese text to analyze (reads stdin if omitted)
    pub text: Option<String>,

    /// Read input from a file instead of the positional argument or stdin
    #[arg(short, long, value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    pub output: OutputFormat,

    /// Path to grammar rule directory
    #[arg(long, default_value = "grammar", value_name = "DIR")]
    pub grammar_db: PathBuf,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    Json,
    Dot,
    /// Print a human-readable token table (useful for exploring tokenizer output)
    Table,
    /// Draw the token chain and grammar annotations as a terminal graph
    Graph,
    /// Dump every raw UniDic field (indices 0–28) for each token — use this to
    /// verify what index maps to what before changing anything in tokenizer.rs
    Raw,
}

impl Cli {
    /// Read the input text from whichever source was specified.
    pub fn read_text(&self) -> Result<String> {
        if let Some(ref text) = self.text {
            return Ok(text.clone());
        }
        if let Some(ref path) = self.file {
            return Ok(std::fs::read_to_string(path)?);
        }
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    }
}
