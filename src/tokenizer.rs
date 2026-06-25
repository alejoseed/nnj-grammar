use anyhow::Result;
use lindera::{
    dictionary::load_dictionary,
    mode::Mode,
    segmenter::Segmenter,
    tokenizer::Tokenizer as LinderaTokenizer,
};
use serde::{Deserialize, Serialize};

/// A single morpheme with UniDic fields.
///
/// UniDic part_of_speech layout (indices into details()):
///   0  = pos1  major POS (品詞):          名詞, 動詞, 助詞, 助動詞, …
///   1  = pos2  subcategory 1 (品詞細分類1): 格助詞, 副助詞, 係助詞, 非自立可能, …
///   2  = pos3  subcategory 2
///   3  = pos4  subcategory 3
///   4  = conj_type  conjugation type (活用型):  五段-カ行, 上一段-ア行, …
///   5  = conj_form  conjugation form (活用形):  連用形-一般, 終止形-一般, …
///   6  = lem_form   lemma form (語彙素読み)
///   7  = lem        lemma (語彙素)
///   8  = orth_base  base orthographic form (書字形基本形) — use this as base_form
///   9  = pron_base  base pronunciation
///   10 = kana_base  base kana form
///   11 = form_base  base form — normalized (語形基本形)
///   (indices 12–28 are prosodic / accent / word-type fields, rarely needed)
///
/// These match what the official Sudachi library returns from part_of_speech()[0..6],
/// dictionary_form(), and normalized_form() — same POS strings, same grammar values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub surface: String,

    // UniDic POS hierarchy
    pub pos1: String,      // major POS
    pub pos2: String,      // subcategory 1
    pub pos3: String,      // subcategory 2
    pub pos4: String,      // subcategory 3

    // Conjugation
    pub conj_type: String, // 活用型
    pub conj_form: String, // 活用形

    // Forms
    pub base_form: String,      // orthographic base (書字形基本形, index 8)
    pub reading: String,        // kana base (index 10)

    // Position in the original text
    pub byte_start: usize,
    pub byte_end: usize,
    pub position: usize,
}

pub struct Tokenizer {
    inner: LinderaTokenizer,
}

impl Tokenizer {
    pub fn new() -> Result<Self> {
        let dictionary = load_dictionary("embedded://unidic")?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        Ok(Self {
            inner: LinderaTokenizer::new(segmenter),
        })
    }

    pub fn tokenize(&self, text: &str) -> Result<Vec<Token>> {
        let mut raw = self.inner.tokenize(text)?;
        let token_surfaces: Vec<&str> = raw.iter().map(|t| t.surface.as_ref()).collect();

        println!("Printing token surfaces: {:?}", token_surfaces);
        
        let tokens = raw
            .iter_mut()
            .enumerate()
            .map(|(position, t)| {
                // details() needs &mut self and returns &str slices borrowed from t.
                // Convert everything to String inside this block before touching
                // any other field on t.
                let owned: Vec<String> = {
                    let d = t.details();
                    (0..29)
                        .map(|i| match d.get(i) {
                            Some(&s) if s != "*" => s.to_string(),
                            _ => String::new(),
                        })
                        .collect()
                };

                let g = |i: usize| owned.get(i).cloned().unwrap_or_default();

                Token {
                    surface: t.surface.to_string(),
                    pos1: g(0),
                    pos2: g(1),
                    pos3: g(2),
                    pos4: g(3),
                    conj_type: g(4),
                    conj_form: g(5),
                    base_form: g(8),   // orth_base = orthographic base form
                    reading: g(10),    // kana_base
                    byte_start: t.byte_start,
                    byte_end: t.byte_end,
                    position,
                }
            })
            .collect();

        Ok(tokens)
    }
}

/// Print a token table — run this first on any sentence before writing grammar rules.
/// The conj_form column shows you the exact UniDic strings to put in your TOML steps.
pub fn print_table(tokens: &[Token]) {
    println!(
        "{:<14} {:<10} {:<14} {:<22} {:<16}",
        "Surface", "POS1", "POS2", "ConjForm", "BaseForm"
    );
    println!("{}", "─".repeat(78));
    for t in tokens {
        println!(
            "{:<14} {:<10} {:<14} {:<22} {:<16}",
            t.surface, t.pos1, t.pos2, t.conj_form, t.base_form
        );
    }
}
