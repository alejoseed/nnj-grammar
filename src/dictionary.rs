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
    /// and auxiliaries get a restricted lookup (surface and lemma, but
    /// particle/auxiliary senses only) so しか glosses to "only" without は
    /// pulling in 歯/葉.
    pub fn lookup_token(&self, token: &Token) -> Vec<DictionaryGloss> {
        if is_function_word(&token.pos1) {
            let keys = function_word_keys(token);
            let mut entries = self.collect_entries(&keys);
            entries.sort_by_key(|entry| !is_headword_match(entry, &keys));
            let glosses = function_word_glosses(&entries, token, MAX_GLOSSES_PER_TOKEN);

            let head_entries: Vec<i64> = entries
                .iter()
                .filter(|entry| is_headword_match(entry, &keys))
                .map(|entry| i64::from(entry.number))
                .collect();
            if glosses.iter().any(|g| head_entries.contains(&g.entry_seq)) {
                return glosses
                    .into_iter()
                    .filter(|g| head_entries.contains(&g.entry_seq))
                    .collect();
            }
            return glosses;
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
fn function_word_glosses(
    entries: &[jmdict::Entry],
    token: &Token,
    limit: usize,
) -> Vec<DictionaryGloss> {
    use jmdict::PartOfSpeech;
    let conjunctive_ok = token.pos2 == "接続助詞";
    let mut glosses: Vec<DictionaryGloss> = Vec::new();
    for entry in entries {
        let mut ranked: Vec<(u8, DictionaryGloss)> = Vec::new();
        for sense in entry.senses() {
            let is_function_sense = sense.parts_of_speech().any(|pos| {
                matches!(
                    pos,
                    PartOfSpeech::Particle
                        | PartOfSpeech::Auxiliary
                        | PartOfSpeech::AuxiliaryVerb
                        | PartOfSpeech::AuxiliaryAdjective
                        | PartOfSpeech::Copula
                        | PartOfSpeech::Expression
                ) || (conjunctive_ok && pos == PartOfSpeech::Conjunction)
            });
            if !is_function_sense {
                continue;
            }
            let text: Vec<&str> = sense.glosses().map(|g| g.text).collect();
            if text.is_empty() {
                continue;
            }
            let gloss = text.join("; ");
            let rank = sense_rank(&sense, token, &gloss);
            let pos: Vec<String> = sense.parts_of_speech().map(|p| format!("{p:?}")).collect();
            ranked.push((
                rank,
                DictionaryGloss {
                    entry_seq: i64::from(entry.number),
                    gloss,
                    pos,
                },
            ));
        }
        ranked.sort_by_key(|(rank, _)| *rank);
        glosses.extend(ranked.into_iter().map(|(_, gloss)| gloss));
        if glosses.len() >= limit {
            break;
        }
    }
    glosses.truncate(limit);
    glosses
}

/// Rank one sense against UniDic's particle class. Lower is better, and 1 is
/// neutral — a class we have no opinion about leaves JMdict's order untouched.
///
/// JMdict lumps every use of a particle into one entry, so 格助詞 の (genitive)
/// and 準体助詞 の (nominalizer) both land on entry 1469800 and the genitive
/// sense wins purely by being listed first. UniDic already drew the
/// distinction; this is where that information gets used.
fn sense_rank(sense: &jmdict::Sense, token: &Token, gloss: &str) -> u8 {
    if let Some(marker) = preferred_gloss_marker(token) {
        return if gloss.contains(marker) { 0 } else { 1 };
    }
    let conjunctive = sense
        .parts_of_speech()
        .any(|pos| pos == jmdict::PartOfSpeech::Conjunction);
    match token.pos2.as_str() {
        // 接続助詞 joins clauses, and JMdict tags exactly those senses
        // Conjunction: が "but; however" over が "indicates subject".
        "接続助詞" if conjunctive => 0,
        // 格助詞 marks a case role and is never a conjunction, so a
        // Conjunction sense is the wrong reading: と "with", not と "if; when".
        "格助詞" if conjunctive => 2,
        _ => 1,
    }
}

/// Senses that UniDic's particle class picks out but JMdict's part-of-speech
/// tags cannot express, so they have to be named. Matched against the joined
/// gloss text; surface is tried before lemma so a contraction can override
/// its lemma's entry where the two genuinely differ.
fn preferred_gloss_marker(token: &Token) -> Option<&'static str> {
    let by_key = |key: &str| match (key, token.pos2.as_str()) {
        // 準体助詞 *is* the nominalizing particle, so the tag maps straight
        // onto the sense — for ん (< の) in んだ/んです just as much as for
        // の in 泳ぐのが. The genitive reading is 格助詞.
        ("の" | "ん", "準体助詞") => Some("nominalizes"),
        // 接続助詞 から is causal (寒いから); 格助詞 から is ablative.
        ("から", "接続助詞") => Some("because"),
        _ => None,
    };
    by_key(&token.surface).or_else(|| by_key(&token.base_form))
}

/// Is one of `keys` this entry's headword — its first kanji element, or its
/// first reading element? JMdict orders both most-standard first, so anything
/// later is a variant spelling of a word the entry is not primarily about.
fn is_headword_match(entry: &jmdict::Entry, keys: &[&str]) -> bool {
    let head_kanji = entry.kanji_elements().next().map(|k| k.text);
    let head_reading = entry.reading_elements().next().map(|r| r.text);
    [head_kanji, head_reading]
        .into_iter()
        .flatten()
        .any(|head| keys.contains(&head))
}

fn reading_matches(entry: jmdict::Entry, reading: &str) -> bool {
    reading.is_empty() || entry.reading_elements().any(|r| r.text == reading)
}

/// Particles and auxiliaries: no full lexical lookup, but JMdict still has
/// real senses for them (しか "only", ない "not").
fn is_function_word(pos1: &str) -> bool {
    matches!(pos1, "助詞" | "助動詞")
}

/// Lookup keys for a function word, strongest first; always search lemma and surface.
/// Prefer the UniDic lemma to avoid homograph traps, except for 融合, 意志推量形,
/// and 仮定形, where the JMdict-lexicalized surface takes priority.
fn function_word_keys(token: &Token) -> [&str; 2] {
    let surface = token.surface.as_str();
    let lemma = token.base_form.as_str();
    let lexicalized = token.conj_form.contains("融合")
        || token.conj_form.starts_with("仮定形")
        || token.conj_form == "意志推量形";
    if lexicalized {
        [surface, lemma]
    } else {
        [lemma, surface]
    }
}

/// True for words that carry lexical meaning worth a dictionary lookup.
/// Skips particles, auxiliaries, symbols, and whitespace (UniDic pos1).
fn is_content_word(pos1: &str) -> bool {
    !matches!(pos1, "助詞" | "助動詞" | "補助記号" | "記号" | "空白" | "")
}
