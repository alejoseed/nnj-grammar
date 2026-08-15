//! Offline English glosses from embedded JMdict.
//!
//! The `jmdict` crate bakes the dictionary into the binary at compile time,
//! the same way `lindera`'s `embed-unidic` feature embeds UniDic. At startup we
//! build an in-memory index keyed by every kanji and reading form so per-token
//! lookup is a hash hit, not a scan of the whole dictionary.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::analysis::DictionaryGloss;
use crate::chunker::SentenceChunk;
use crate::tokenizer::Token;

/// Maximum gloss senses attached to a single token, to keep payloads small.
const MAX_GLOSSES_PER_TOKEN: usize = 6;


pub struct Dictionary {
    /// Kanji text or reading text -> the entries that use it. Keys borrow the
    /// crate's embedded `'static` strings, so no per-key allocation happens.
    index: HashMap<&'static str, Vec<jmdict::Entry>>,
}

impl Dictionary {
    /// The process-wide embedded dictionary, built once on first use. JMdict is
    /// immutable global data, so every `Analyzer` shares one index rather than
    /// rebuilding its ~200k-entry map per construction.
    pub fn shared() -> &'static Dictionary {
        static SHARED: OnceLock<Dictionary> = OnceLock::new();
        SHARED.get_or_init(Dictionary::embedded)
    }

    /// Build the lookup index from the embedded JMdict data. Prefer `shared()`;
    /// this is public mainly for the one-time initialization it backs.
    pub fn embedded() -> Self {
        let mut index: HashMap<&'static str, Vec<jmdict::Entry>> = HashMap::new();
        for entry in jmdict::entries() {
            for kanji in entry.kanji_elements() {
                index.entry(kanji.text).or_default().push(entry);
            }
            for reading in entry.reading_elements() {
                index.entry(reading.text).or_default().push(entry);
            }
        }
        Self { index }
    }

    /// Gloss a whole token sequence at once. Each token gets its own glosses,
    /// then a compound pass fuses adjacent content tokens whose joined surface
    /// is a JMdict entry (図書 + 館 -> 図書館 "library") and prepends that
    /// compound gloss to every token in the span. The bunsetsu end bounds the
    /// search window; the content-word check delimits the word itself.
    ///
    /// Also returns the fused spans (inclusive) — each is one dictionary word
    /// that UniDic split into short units, which the tree renders as a single
    /// 単語 node.
    pub fn gloss_tokens(
        &self,
        tokens: &[Token],
        sentences: &[SentenceChunk],
    ) -> (Vec<Vec<DictionaryGloss>>, Vec<(usize, usize)>) {
        let mut per_token: Vec<Vec<DictionaryGloss>> =
            tokens.iter().map(|token| self.lookup_token(token)).collect();

        // Position -> inclusive end of its bunsetsu.
        let mut bunsetsu_end = vec![0usize; tokens.len()];
        for sentence in sentences {
            for chunk in &sentence.bunsetsu {
                for position in chunk.token_start..=chunk.token_end {
                    bunsetsu_end[position] = chunk.token_end;
                }
            }
        }

        let mut words = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            let mut consumed = 1;
            if is_content_word(&tokens[i].pos1) {
                let max_len = bunsetsu_end[i] - i + 1;
                // Prefer the longest compound that resolves.
                for len in (2..=max_len).rev() {
                    let span = &tokens[i..i + len];
                    if !span.iter().all(|t| is_content_word(&t.pos1)) {
                        continue;
                    }
                    let surface: String = span.iter().map(|t| t.surface.as_str()).collect();
                    let Some(hits) = self.index.get(surface.as_str()) else {
                        continue;
                    };
                    let compound = entries_to_glosses(hits, 2);
                    if compound.is_empty() {
                        continue;
                    }
                    for slot in &mut per_token[i..i + len] {
                        let mut merged = compound.clone();
                        merged.append(slot);
                        merged.truncate(MAX_GLOSSES_PER_TOKEN);
                        *slot = merged;
                    }
                    words.push((i, i + len - 1));
                    consumed = len;
                    break;
                }
            }
            i += consumed;
        }

        (per_token, words)
    }

    /// Look up English glosses for one token. Punctuation is skipped; particles
    /// and auxiliaries get a restricted lookup (surface only, particle/auxiliary
    /// senses only) so しか glosses to "only" without は pulling in 歯/葉.
    pub fn lookup_token(&self, token: &Token) -> Vec<DictionaryGloss> {
        if is_function_word(&token.pos1) {
            let entries = self.collect_entries(&[token.surface.as_str()]);
            return function_word_glosses(&entries, MAX_GLOSSES_PER_TOKEN);
        }
        if !is_content_word(&token.pos1) {
            return Vec::new();
        }

        // Try the dictionary form first (best for conjugated verbs/adjectives),
        // then the surface, then the kana reading.
        let mut entries = self.collect_entries(&[
            token.base_form.as_str(),
            token.surface.as_str(),
            token.reading.as_str(),
        ]);

        // The reading key drags in homophones (わたし also hits 渡し "ferry").
        // The UniDic lemma is the stronger signal: when any entry writes the
        // lemma, keep only those. Fail open when none do, so lemmas JMdict
        // spells differently still gloss via surface/reading.
        let lemma = token.base_form.as_str();
        if !lemma.is_empty() {
            let writes_lemma = |entry: &jmdict::Entry| {
                entry.kanji_elements().any(|kanji| kanji.text == lemma)
                    || entry.reading_elements().any(|reading| reading.text == lemma)
            };
            if entries.iter().any(writes_lemma) {
                entries.retain(writes_lemma);
            }
        }

        // Prefer entries whose reading matches the token reading (disambiguates
        // homographs like 行った/行く vs 行う).
        entries.sort_by_key(|entry| !reading_matches(*entry, &token.reading));

        entries_to_glosses(&entries, MAX_GLOSSES_PER_TOKEN)
    }

    /// Gather the distinct entries indexed under any of `keys`, in key order.
    fn collect_entries(&self, keys: &[&str]) -> Vec<jmdict::Entry> {
        let mut entries: Vec<jmdict::Entry> = Vec::new();
        let mut seen_numbers: Vec<u32> = Vec::new();
        for key in keys {
            if key.is_empty() {
                continue;
            }
            if let Some(hits) = self.index.get(*key) {
                for &entry in hits {
                    if !seen_numbers.contains(&entry.number) {
                        seen_numbers.push(entry.number);
                        entries.push(entry);
                    }
                }
            }
        }
        entries
    }
}

/// Turn up to `limit` senses of the given entries into display glosses.
fn entries_to_glosses(entries: &[jmdict::Entry], limit: usize) -> Vec<DictionaryGloss> {
    let mut glosses = Vec::new();
    for entry in entries {
        for sense in entry.senses() {
            let text: Vec<&str> = sense.glosses().map(|g| g.text).collect();
            if text.is_empty() {
                continue;
            }
            let pos: Vec<String> = sense.parts_of_speech().map(|p| format!("{p:?}")).collect();
            glosses.push(DictionaryGloss {
                entry_seq: i64::from(entry.number),
                gloss: text.join("; "),
                pos,
            });
            if glosses.len() >= limit {
                return glosses;
            }
        }
    }
    glosses
}

/// Glosses for a function word: only senses JMdict itself marks as a particle
/// or auxiliary. Homophone content senses (歯 for は) never qualify.
fn function_word_glosses(entries: &[jmdict::Entry], limit: usize) -> Vec<DictionaryGloss> {
    use jmdict::PartOfSpeech;
    let mut glosses = Vec::new();
    for entry in entries {
        for sense in entry.senses() {
            let is_function_sense = sense.parts_of_speech().any(|pos| {
                matches!(
                    pos,
                    PartOfSpeech::Particle
                        | PartOfSpeech::Auxiliary
                        | PartOfSpeech::AuxiliaryVerb
                        | PartOfSpeech::AuxiliaryAdjective
                )
            });
            if !is_function_sense {
                continue;
            }
            let text: Vec<&str> = sense.glosses().map(|g| g.text).collect();
            if text.is_empty() {
                continue;
            }
            let pos: Vec<String> = sense.parts_of_speech().map(|p| format!("{p:?}")).collect();
            glosses.push(DictionaryGloss {
                entry_seq: i64::from(entry.number),
                gloss: text.join("; "),
                pos,
            });
            if glosses.len() >= limit {
                return glosses;
            }
        }
    }
    glosses
}

fn reading_matches(entry: jmdict::Entry, reading: &str) -> bool {
    reading.is_empty() || entry.reading_elements().any(|r| r.text == reading)
}

/// Particles and auxiliaries: no full lexical lookup, but JMdict still has
/// real senses for them (しか "only", ない "not").
fn is_function_word(pos1: &str) -> bool {
    matches!(pos1, "助詞" | "助動詞")
}

/// True for words that carry lexical meaning worth a dictionary lookup.
/// Skips particles, auxiliaries, symbols, and whitespace (UniDic pos1).
fn is_content_word(pos1: &str) -> bool {
    !matches!(pos1, "助詞" | "助動詞" | "補助記号" | "記号" | "空白" | "")
}
