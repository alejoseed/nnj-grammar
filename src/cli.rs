use anyhow::Result;
use clap::Parser;
use std::io::Read;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "nnj-grammar",
    version,
    about = "Japanese grammar pattern analyzer",
    long_about = "\
Tokenizes Japanese text and identifies grammar constructions (JLPT N5–N1).
No internet connection required. No LLM. Runs offline from a single binary.

Each token is analyzed using the UniDic dictionary (embedded at compile time).
Grammar patterns are loaded from TOML files in the grammar/ directory.",
    after_help = "\
EXAMPLES:
  # See the grammar graph in the terminal
  nnj-grammar --output graph \"東京しか行かない\"

  # See how the tokenizer splits a sentence (useful when writing grammar rules)
  nnj-grammar --output table \"食べている\"

  # See all raw UniDic fields for each token (use this to verify field indices)
  nnj-grammar --output raw \"行かない\"

  # Get the full analysis document (tokens + ranked matches + node/edge tree)
  nnj-grammar --output json \"私はコタローです\"

  # See the graph's real parent/child shape — same tree the web UI renders
  nnj-grammar --output tree \"わたしは\"

  # Read from a file
  nnj-grammar --output graph --file sentence.txt

  # Use a custom grammar rule directory
  nnj-grammar --output graph --grammar-db ./my-rules \"コーヒーを飲む\""
)]
pub struct Cli {
    /// Japanese text to analyze
    pub text: Option<String>,

    /// Read input from a file
    #[arg(short, long, value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// How to display the results
    #[arg(short, long, value_enum, default_value = "graph")]
    pub output: OutputFormat,

    /// Directory containing grammar rule TOML files
    #[arg(long, default_value = "grammar", value_name = "DIR")]
    pub grammar_db: PathBuf,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    /// Terminal graph — token chain with grammar annotations below (default)
    Graph,
    /// Full AnalysisDocument JSON — tokens, ranked matches, and the node/edge tree
    Json,
    /// Indented node/edge tree — the exact hierarchy the web UI renders
    Tree,
    /// 文/文節 chunking — deterministic structure from POS alone, no grammar rules
    Bunsetsu,
    /// Chunking trace — every attach/split decision with the rule that made it
    BunsetsuTrace,
    /// Token table — surface, reading, POS, conjugation form, base form
    Table,
    /// Raw UniDic fields — all 29 indices numbered, for verifying what each index means
    Raw,
    /// Token array as JSON, without loading or matching grammar rules
    Tokens,
    /// DOT format — for Graphviz rendering
    Dot,
}

impl Cli {
    /// Read the input text, or return None if no input was provided.
    pub fn read_text(&self) -> Result<Option<String>> {
        if let Some(ref text) = self.text {
            return Ok(Some(text.clone()));
        }
        if let Some(ref path) = self.file {
            return Ok(Some(std::fs::read_to_string(path)?));
        }
        // Only read stdin if it's being piped (not an interactive terminal)
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            if !buf.trim().is_empty() {
                return Ok(Some(buf));
            }
        }
        Ok(None)
    }
}
