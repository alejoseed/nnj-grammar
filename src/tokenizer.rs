use anyhow::Result;
use lindera::{
    dictionary::load_dictionary, mode::Mode, segmenter::Segmenter,
    tokenizer::Tokenizer as LinderaTokenizer,
};
use serde::{Deserialize, Serialize};

/// A single morpheme with UniDic fields.
///
/// Field layout of the embedded UniDic, as `--output raw` prints it
/// (indices into details()):
///   0  = pos1  major POS (品詞):          名詞, 動詞, 助詞, 助動詞, …
///   1  = pos2  subcategory 1 (品詞細分類1): 格助詞, 副助詞, 係助詞, 非自立可能, …
///   2  = pos3  subcategory 2
///   3  = pos4  subcategory 3
///   4  = conj_type  conjugation type (活用型):  五段-カ行, 上一段-ア行, …
///   5  = conj_form  conjugation form (活用形):  連用形-一般, 終止形-一般, …
///   6  = lemma reading (語彙素読み), katakana:           イク for 行か
///   7  = lemma (語彙素), may carry a suffix:              行く, 私-代名詞, 箇月 for ヶ月
///   8  = surface orthography (書字形出現形), the surface:  行か
///   9  = surface pronunciation (発音形出現形), ー for long vowels: イカ, トーキョー
///   10 = orthographic base (書字形基本形), spelled like the surface: 行く, ヶ月
///   11 = base pronunciation (発音形基本形)
///   12 = word origin (語種): 和, 漢, 外, …
///   13–16 appear on some tokens only (sound-change type and form: ホ濁, チ促);
///   Indices past 16, including the kana surface field 20, are absent.
///   Punctuation and symbols carry no 6, 9, or 11.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub surface: String,

    // UniDic POS hierarchy
    pub pos1: String, // major POS
    pub pos2: String, // subcategory 1
    pub pos3: String, // subcategory 2
    pub pos4: String, // subcategory 3

    // Conjugation
    pub conj_type: String, // 活用型
    pub conj_form: String, // 活用形

    // Forms
    pub base_form: String,
    pub reading: String,
    pub pronunciation: String,
    pub orth_base: String,

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
                    base_form: g(7), // 語彙素 — true dictionary/lemma form (行く not 行か, する not し)
                    reading: katakana_to_hiragana(&g(6)), // 語彙素読み — katakana reading converted to hiragana
                    pronunciation: katakana_to_hiragana(&g(9)),
                    orth_base: g(10),
                    byte_start: t.byte_start,
                    byte_end: t.byte_end,
                    position,
                }
            })
            .collect();

        Ok(tokens)
    }
}

/// Convert a katakana string to hiragana for furigana display.
pub(crate) fn katakana_to_hiragana(s: &str) -> String {
    s.chars().map(hiragana_of).collect()
}

/// Each katakana character (U+30A1–U+30F6) maps to hiragana by subtracting 0x60.
pub(crate) fn hiragana_of(c: char) -> char {
    match u32::from(c) {
        n @ 0x30A1..=0x30F6 => char::from_u32(n - 0x60).unwrap_or(c),
        _ => c,
    }
}

/// Dump every raw UniDic field (indices 0–28) for each token.
/// Use `--output raw` to call this. Read the index numbers yourself and decide
/// which ones to use — don't rely on comments in the code.
pub fn print_raw(tokenizer: &Tokenizer, text: &str) -> Result<()> {
    let mut raw = tokenizer.inner.tokenize(text)?;
    for t in raw.iter_mut() {
        let surface = t.surface.to_string();
        let details = t.details();
        println!("── {} ──", surface);
        for (i, val) in details.iter().enumerate() {
            if *val != "*" && !val.is_empty() {
                println!("  [{:>2}]  {}", i, val);
            }
        }
        println!();
    }
    Ok(())
}

/// Print a token table — run this first on any sentence before writing grammar rules.
/// The conj_form column shows you the exact UniDic strings to put in your TOML steps.
pub fn print_table(tokens: &[Token]) {
    println!(
        "{:<14} {:<16} {:<12} {:<14} {:<22} {:<14}",
        "Surface", "Reading ふりがな", "POS 品詞", "Sub 細分類", "ConjForm 活用形", "Base 原形"
    );
    println!("{}", "─".repeat(95));
    for t in tokens {
        println!(
            "{:<14} {:<16} {:<12} {:<14} {:<22} {:<14}",
            t.surface, t.reading, t.pos1, t.pos2, t.conj_form, t.base_form
        );
    }

    println!();
    println!("POS guide — 品詞 (ひんし) = part of speech:");
    println!("  名詞 めいし       noun");
    println!("  動詞 どうし       verb");
    println!("  助詞 じょし       particle");
    println!("  助動詞 じょどうし  auxiliary verb");
    println!("  形容詞 けいようし  i-adjective");
    println!("  副詞 ふくし       adverb");
    println!();
    println!("Sub (細分類) guide:");
    println!("  格助詞 かくじょし  case particle  (が、を、に、で、から…)");
    println!("  係助詞 かかりじょし topic particle  (は、も)");
    println!("  副助詞 ふくじょし  adverbial particle (しか、だけ、まで…)");
    println!("  接続助詞 せつぞくじょし conjunctive particle (て、ば、と…)");
    println!("  非自立可能 ひじりつかのう  can work as auxiliary (いる、くる…)");
    println!("  固有名詞 こゆうめいし  proper noun");
    println!("  普通名詞 ふつうめいし  common noun");
}
