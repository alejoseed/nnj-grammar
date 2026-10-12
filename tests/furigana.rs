#![allow(clippy::panic, reason = "test helpers fail loudly by design")]

use nnj_grammar::analysis::AnalysisDocument;
use nnj_grammar::analyzer::{Analyzer, AnalyzerConfig};

fn analyze(analyzer: &Analyzer, input: &str) -> AnalysisDocument {
    analyzer
        .analyze(input)
        .unwrap_or_else(|error| panic!("{input} should analyze: {error}"))
}

fn notation(document: &AnalysisDocument) -> String {
    document
        .tokens
        .iter()
        .map(|token| {
            token
                .furigana
                .iter()
                .map(|segment| match &segment.reading {
                    Some(reading) => format!("{}[{reading}]", segment.text),
                    None => segment.text.clone(),
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("|")
}

#[test]
fn furigana_follows_the_surface_form() {
    let analyzer = Analyzer::new(AnalyzerConfig::default()).expect("analyzer initializes");
    let cases = [
        ("東京しか行かない", "東京[とうきょう]|しか|行[い]か|ない"),
        (
            "自動販売機で水を買った",
            "自動[じどう]|販売[はんばい]|機[き]|で|水[みず]|を|買[か]っ|た",
        ),
        (
            "取り消した美しい絵",
            "取[と]り消[け]し|た|美[うつく]しい|絵[え]",
        ),
        ("来た", "来[き]|た"),
        ("来ない", "来[こ]|ない"),
        ("承りました", "承[うけたまわ]り|まし|た"),
        ("一ヶ月", "一[いち]|ヶ月[かげつ]"),
        ("コーヒーを飲む", "コーヒー|を|飲[の]む"),
        ("3月に行きます", "3|月[がつ]|に|行[い]き|ます"),
        ("30日", "3|0|日[にち]"),
        ("何の本", "何[なん]|の|本[ほん]"),
        ("言う", "言[い]う"),
        ("彼はそう言った", "彼[かれ]|は|そう|言[い]っ|た"),
        ("続く", "続[つづ]く"),
        ("鼻血", "鼻血[はなぢ]"),
        ("縮む", "縮[ちぢ]む"),
        ("大きかった", "大[おお]きかっ|た"),
        ("通る", "通[とお]る"),
        ("行こう", "行[い]こう"),
        ("下さい", "下[くだ]さい"),
        ("話し合い", "話[はな]し合[あ]い"),
        ("引き出し", "引[ひ]き出[だ]し"),
        (
            "今日は大人が一人",
            "今日[きょう]|は|大人[おとな]|が|一人[ひとり]",
        ),
        ("関ヶ原", "関ヶ原[せきがはら]"),
        ("人々", "人々[ひとびと]"),
        ("お茶", "お|茶[ちゃ]"),
        ("二十日", "二十[はつ]|日[か]"),
        ("私は学生です", "私[わたくし]|は|学生[がくせい]|です"),
        ("東京へ行く。", "東京[とうきょう]|へ|行[い]く|。"),
        (
            "彼は走って帰った",
            "彼[かれ]|は|走[はし]っ|て|帰[かえ]っ|た",
        ),
        ("本を読んだ", "本[ほん]|を|読[よ]ん|だ"),
        ("泳いだ", "泳[およ]い|だ"),
        ("寒くない", "寒[さむ]く|ない"),
        ("美しかった", "美[うつく]しかっ|た"),
        ("話せる", "話[はな]せる"),
        ("来い", "来[こ]い"),
        ("来れば", "来[く]れ|ば"),
        ("来よう", "来[こ]よう"),
        ("十日間", "十[とお]|日間[かかん]"),
        ("二十", "二十[にじゅう]"),
        ("先生", "先生[せんせい]"),
        ("通れる", "通[とお]れる"),
        ("物の怪", "物の怪"),
        ("物の本", "物の本"),
    ];
    for (input, expected) in cases {
        assert_eq!(
            notation(&analyze(&analyzer, input)),
            expected,
            "furigana of {input}"
        );
    }
}

fn is_kana_or_ascii(c: char) -> bool {
    ('\u{3041}'..='\u{309F}').contains(&c)
        || (('\u{30A0}'..='\u{30FF}').contains(&c) && !matches!(c, 'ヵ' | 'ヶ'))
        || c.is_ascii()
}

fn is_hiragana_reading(c: char) -> bool {
    ('\u{3041}'..='\u{3096}').contains(&c) || c == 'ー'
}

#[test]
fn furigana_spells_every_surface_of_long_text() {
    let analyzer = Analyzer::new(AnalyzerConfig::default()).expect("analyzer initializes");
    let text = "今さらながらに思うんだけどさ......相手の顔色窺って様子を見てるだけっていうのは、相手を一番困らせるんだと思う";
    let document = analyze(&analyzer, text);
    let mut ruby_segments = 0;
    for token in &document.tokens {
        let spelled: String = token
            .furigana
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        assert_eq!(spelled, token.surface, "segments of {} spell it", token.id);
        for segment in &token.furigana {
            assert!(!segment.text.is_empty(), "empty segment in {}", token.id);
            if let Some(reading) = &segment.reading {
                ruby_segments += 1;
                assert!(
                    !segment.text.chars().any(is_kana_or_ascii),
                    "ruby over kana or ASCII {:?} in {}",
                    segment.text,
                    token.id
                );
                assert!(
                    !reading.is_empty() && reading.chars().all(is_hiragana_reading),
                    "reading {reading:?} of {} is not hiragana",
                    token.surface
                );
            }
        }
        assert!(
            token
                .furigana
                .windows(2)
                .all(|pair| pair[0].reading.is_some() || pair[1].reading.is_some()),
            "adjacent plain segments in {}",
            token.id
        );
    }
    assert!(
        ruby_segments >= 10,
        "only {ruby_segments} ruby segments in the long text"
    );
}
