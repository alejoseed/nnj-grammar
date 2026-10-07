use schemars::JsonSchema;
use serde::Serialize;

use crate::tokenizer::{hiragana_of, katakana_to_hiragana, Token};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct FuriganaSegment {
    pub text: String,
    pub reading: Option<String>,
}

pub fn furigana(token: &Token) -> Vec<FuriganaSegment> {
    let plain = || {
        vec![FuriganaSegment {
            text: token.surface.clone(),
            reading: None,
        }]
    };
    if !token.surface.chars().any(is_kanji) {
        return plain();
    }
    let surface: Vec<char> = token.surface.chars().collect();
    let reading: Vec<char> = surface_reading(token).chars().collect();
    align(&surface, &reading).unwrap_or_else(plain)
}

fn is_kanji(c: char) -> bool {
    matches!(
        c,
        '々' | '〆'
            | '〇'
            | 'ヵ'
            | 'ヶ'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{3FFFF}'
    )
}

fn surface_reading(token: &Token) -> String {
    match spelled_from_lemma(token) {
        Some(spelled) if sound_key(&spelled) == sound_key(&token.pronunciation) => spelled,
        _ => spell_long_vowels(&token.pronunciation, &token.reading),
    }
}

fn spelled_from_lemma(token: &Token) -> Option<String> {
    let surface: Vec<char> = token.surface.chars().collect();
    let base: Vec<char> = token.orth_base.chars().collect();
    let stem = surface
        .iter()
        .zip(&base)
        .take_while(|(s, b)| s == b)
        .count();
    let surface_tail: String = surface[stem..].iter().collect();
    let base_tail: String = base[stem..].iter().collect();
    if surface_tail.chars().chain(base_tail.chars()).any(is_kanji) {
        return None;
    }
    let stem_reading = token
        .reading
        .strip_suffix(katakana_to_hiragana(&base_tail).as_str())?;
    Some(format!(
        "{stem_reading}{}",
        katakana_to_hiragana(&surface_tail)
    ))
}

fn sound_key(kana: &str) -> Vec<char> {
    let chars: Vec<char> = kana.chars().collect();
    let mut previous: Option<Vowel> = None;
    chars
        .iter()
        .enumerate()
        .map(|(index, &c)| {
            let c = match c {
                'づ' => 'ず',
                'ぢ' => 'じ',
                'い' if chars.get(index + 1) == Some(&'う') => 'ゆ',
                other => other,
            };
            if c == 'ー' || lengthens(previous, c) {
                'ー'
            } else {
                previous = vowel(c);
                c
            }
        })
        .collect()
}

fn lengthens(previous: Option<Vowel>, c: char) -> bool {
    matches!(
        (previous, c),
        (Some(Vowel::O | Vowel::U), 'う')
            | (Some(Vowel::E | Vowel::I), 'い')
            | (Some(Vowel::A), 'あ')
            | (Some(Vowel::E), 'え')
            | (Some(Vowel::O), 'お')
    )
}

fn spell_long_vowels(pronunciation: &str, lemma_reading: &str) -> String {
    let said = sound_key(pronunciation);
    let lemma: Vec<char> = lemma_reading.chars().collect();
    let lemma_key = sound_key(lemma_reading);
    let mut spelled = String::new();
    let mut previous: Option<Vowel> = None;
    for (index, c) in pronunciation.chars().enumerate() {
        if c != 'ー' {
            spelled.push(c);
            previous = vowel(c);
            continue;
        }
        let row_spelling = match previous {
            Some(Vowel::A) => 'あ',
            Some(Vowel::I | Vowel::E) => 'い',
            Some(Vowel::U | Vowel::O) => 'う',
            None => 'ー',
        };
        let lemma_spelling = match (lemma_key.get(..=index), said.get(..=index)) {
            (Some(lemma_sounds), Some(said_sounds)) if lemma_sounds == said_sounds => {
                lemma.get(index).copied()
            }
            _ => None,
        };
        spelled.push(lemma_spelling.unwrap_or(row_spelling));
    }
    spelled
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vowel {
    A,
    I,
    U,
    E,
    O,
}

fn vowel(c: char) -> Option<Vowel> {
    match c {
        'あ' | 'か' | 'が' | 'さ' | 'ざ' | 'た' | 'だ' | 'な' | 'は' | 'ば' | 'ぱ' | 'ま'
        | 'や' | 'ら' | 'わ' | 'ぁ' | 'ゃ' | 'ゎ' | 'ゕ' => Some(Vowel::A),
        'い' | 'き' | 'ぎ' | 'し' | 'じ' | 'ち' | 'ぢ' | 'に' | 'ひ' | 'び' | 'ぴ' | 'み'
        | 'り' | 'ゐ' | 'ぃ' => Some(Vowel::I),
        'う' | 'く' | 'ぐ' | 'す' | 'ず' | 'つ' | 'づ' | 'ぬ' | 'ふ' | 'ぶ' | 'ぷ' | 'む'
        | 'ゆ' | 'る' | 'ぅ' | 'ゅ' | 'ゔ' => Some(Vowel::U),
        'え' | 'け' | 'げ' | 'せ' | 'ぜ' | 'て' | 'で' | 'ね' | 'へ' | 'べ' | 'ぺ' | 'め'
        | 'れ' | 'ゑ' | 'ぇ' | 'ゖ' => Some(Vowel::E),
        'お' | 'こ' | 'ご' | 'そ' | 'ぞ' | 'と' | 'ど' | 'の' | 'ほ' | 'ぼ' | 'ぽ' | 'も'
        | 'よ' | 'ろ' | 'を' | 'ぉ' | 'ょ' => Some(Vowel::O),
        _ => None,
    }
}

fn align(surface: &[char], reading: &[char]) -> Option<Vec<FuriganaSegment>> {
    let Some((&head, after)) = surface.split_first() else {
        return reading.is_empty().then(Vec::new);
    };
    if !is_kanji(head) {
        let (&said, tail) = reading.split_first()?;
        if said != hiragana_of(head) {
            return None;
        }
        let mut segments = align(after, tail)?;
        match segments.first_mut() {
            Some(FuriganaSegment {
                text,
                reading: None,
            }) => text.insert(0, head),
            _ => segments.insert(
                0,
                FuriganaSegment {
                    text: head.to_string(),
                    reading: None,
                },
            ),
        }
        return Some(segments);
    }
    let (run, rest) = surface.split_at(surface.iter().take_while(|&&c| is_kanji(c)).count());
    (1..=reading.len()).find_map(|length| {
        let (said, tail) = reading.split_at(length);
        let mut segments = align(rest, tail)?;
        segments.insert(
            0,
            FuriganaSegment {
                text: run.iter().collect(),
                reading: Some(said.iter().collect()),
            },
        );
        Some(segments)
    })
}

#[cfg(test)]
mod tests {
    use super::{furigana, FuriganaSegment};
    use crate::tokenizer::Token;

    fn token(surface: &str, reading: &str, pronunciation: &str, orth_base: &str) -> Token {
        Token {
            surface: surface.to_string(),
            pos1: String::new(),
            pos2: String::new(),
            pos3: String::new(),
            pos4: String::new(),
            conj_type: String::new(),
            conj_form: String::new(),
            base_form: String::new(),
            reading: reading.to_string(),
            pronunciation: pronunciation.to_string(),
            orth_base: orth_base.to_string(),
            byte_start: 0,
            byte_end: surface.len(),
            position: 0,
        }
    }

    fn notation(segments: &[FuriganaSegment]) -> String {
        segments
            .iter()
            .map(|segment| match &segment.reading {
                Some(reading) => format!("{}[{reading}]", segment.text),
                None => segment.text.clone(),
            })
            .collect()
    }

    fn read(surface: &str, reading: &str, pronunciation: &str, orth_base: &str) -> String {
        notation(&furigana(&token(
            surface,
            reading,
            pronunciation,
            orth_base,
        )))
    }

    #[test]
    fn spelled_lemma_wins_when_it_sounds_like_the_surface() {
        assert_eq!(
            read("東京", "とうきょう", "とーきょー", "東京"),
            "東京[とうきょう]"
        );
    }

    #[test]
    fn pronunciation_wins_when_the_lemma_form_differs() {
        assert_eq!(read("何", "なに", "なん", "何"), "何[なん]");
    }

    #[test]
    fn irregular_stem_falls_back_to_the_pronunciation() {
        assert_eq!(read("来", "くる", "き", "来る"), "来[き]");
    }

    #[test]
    fn iu_is_said_yuu() {
        assert_eq!(read("言う", "いう", "ゆー", "言う"), "言[い]う");
    }

    #[test]
    fn dzu_and_dji_are_said_zu_and_ji() {
        assert_eq!(read("続く", "つづく", "つずく", "続く"), "続[つづ]く");
        assert_eq!(read("鼻血", "はなぢ", "はなじ", "鼻血"), "鼻血[はなぢ]");
    }

    #[test]
    fn base_spelled_differently_uses_the_pronunciation() {
        assert_eq!(
            read("取消し", "とりけす", "とりけし", "取り消す"),
            "取消[とりけ]し"
        );
    }

    #[test]
    fn pronunciation_long_vowels_are_spelled_out_in_fallback() {
        assert_eq!(read("東京", "", "とーきょー", ""), "東京[とうきょう]");
    }

    #[test]
    fn e_row_and_plain_long_vowels_in_fallback() {
        assert_eq!(read("映画", "", "えーが", ""), "映画[えいが]");
        assert_eq!(read("母", "", "かー", ""), "母[かあ]");
    }

    #[test]
    fn fallback_spells_a_long_vowel_the_way_the_lemma_does() {
        assert_eq!(
            read("通れる", "とおる", "とーれる", "通れる"),
            "通[とお]れる"
        );
    }

    #[test]
    fn fallback_uses_the_row_rule_once_the_lemma_sounds_different() {
        assert_eq!(read("来よう", "くる", "こよー", "来る"), "来[こ]よう");
    }

    #[test]
    fn katakana_anchor_matches_its_hiragana_reading() {
        assert_eq!(
            furigana(&token("カ月", "かげつ", "かげつ", "カ月")),
            [
                FuriganaSegment {
                    text: "カ".to_string(),
                    reading: None
                },
                FuriganaSegment {
                    text: "月".to_string(),
                    reading: Some("げつ".to_string())
                },
            ]
        );
    }

    #[test]
    fn kana_after_a_kanji_anchors_its_reading() {
        assert_eq!(read("行か", "", "いか", ""), "行[い]か");
    }

    #[test]
    fn unanchorable_reading_yields_no_furigana() {
        assert_eq!(
            furigana(&token("行か", "", "いき", "")),
            [FuriganaSegment {
                text: "行か".to_string(),
                reading: None
            }]
        );
    }

    #[test]
    fn unknown_word_yields_no_furigana() {
        assert_eq!(read("𩸽", "ほっけ", "ほっけ", "𩸽"), "𩸽[ほっけ]");
        assert_eq!(
            furigana(&token("𩸽", "", "", "")),
            [FuriganaSegment {
                text: "𩸽".to_string(),
                reading: None
            }]
        );
    }

    #[test]
    fn kana_get_no_furigana_but_marks_read_like_kanji_do() {
        assert_eq!(
            furigana(&token("しか", "しか", "しか", "しか")),
            [FuriganaSegment {
                text: "しか".to_string(),
                reading: None
            }]
        );
        assert_eq!(
            furigana(&token("コーヒー", "こーひー", "こーひー", "コーヒー")),
            [FuriganaSegment {
                text: "コーヒー".to_string(),
                reading: None
            }]
        );
        assert_eq!(read("〆", "しめ", "しめ", "〆"), "〆[しめ]");
        assert_eq!(
            read("人々", "ひとびと", "ひとびと", "人々"),
            "人々[ひとびと]"
        );
        assert_eq!(read("ヶ月", "かげつ", "かげつ", "ヶ月"), "ヶ月[かげつ]");
    }

    #[test]
    fn mismatched_tail_kana_rejects_the_candidate() {
        assert_eq!(read("行か", "いく", "ゆか", "行く"), "行[ゆ]か");
    }
}
