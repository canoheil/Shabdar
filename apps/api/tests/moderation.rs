//! Phase 10 pure literal normalization and false-positive-sensitive matching.

use shabdar_api::moderation::{matching_rules, normalize_rule, MatchMode, ModerationRule};

fn matches(phrase: &str, body: &str, mode: MatchMode) -> bool {
    let rule = rule("test", phrase, mode);
    !matching_rules(body, &[rule]).is_empty()
}

fn rule(public_id: &str, phrase: &str, mode: MatchMode) -> ModerationRule {
    let normalized = normalize_rule(phrase, mode).expect("valid test rule");
    ModerationRule {
        public_id: public_id.to_owned(),
        phrase: normalized.phrase,
        normalized_phrase: normalized.normalized_phrase,
        revision: 1,
        match_mode: mode,
    }
}

#[test]
fn match_mode_parsing_is_exact() {
    assert_eq!(MatchMode::parse("phrase"), Some(MatchMode::Phrase));
    assert_eq!(MatchMode::parse("compact"), Some(MatchMode::Compact));
    assert_eq!(MatchMode::parse("regex"), None);
    assert_eq!(MatchMode::parse("Phrase"), None);
    assert_eq!(MatchMode::Phrase.as_str(), "phrase");
    assert_eq!(MatchMode::Compact.as_str(), "compact");
}

#[test]
fn rule_validation_uses_unicode_scalar_limits_and_normalized_alphanumerics() {
    assert_eq!(normalize_rule("", MatchMode::Phrase), Err("required"));
    assert_eq!(normalize_rule(" \t\n", MatchMode::Phrase), Err("required"));
    assert_eq!(normalize_rule("!!!", MatchMode::Compact), Err("too_short"));
    assert_eq!(normalize_rule("آ", MatchMode::Phrase), Err("too_short"));

    let valid_120 = "آب".repeat(60);
    assert_eq!(valid_120.chars().count(), 120);
    assert!(normalize_rule(&valid_120, MatchMode::Phrase).is_ok());
    let too_long = format!("{valid_120}آ");
    assert_eq!(too_long.chars().count(), 121);
    assert_eq!(
        normalize_rule(&too_long, MatchMode::Compact),
        Err("too_long")
    );
}

#[test]
fn normalization_maps_persian_arabic_letters_ascii_case_and_repetition() {
    let phrase = normalize_rule("  Foo, BAR  ", MatchMode::Phrase).unwrap();
    assert_eq!(phrase.phrase, "Foo, BAR");
    assert_eq!(phrase.normalized_phrase, "fo bar");
    assert_eq!(phrase.dedupe_key, "fo bar");

    let compact = normalize_rule("Foo, BAR", MatchMode::Compact).unwrap();
    assert_eq!(compact.normalized_phrase, "fobar");
    assert_eq!(compact.dedupe_key, "fo bar");

    let mapped = normalize_rule("كك يي ىى", MatchMode::Phrase).unwrap();
    assert_eq!(mapped.normalized_phrase, "ک ی ی");
    let repeated = normalize_rule("heeello", MatchMode::Compact).unwrap();
    assert_eq!(repeated.normalized_phrase, "helo");
    assert!(matches("bad", "BAAD", MatchMode::Compact));
}

#[test]
fn every_specified_diacritic_is_removed_without_adding_a_separator() {
    let points: Vec<u32> = (0x064B..=0x065F)
        .chain(0x0670..=0x0670)
        .chain(0x06D6..=0x06ED)
        .collect();
    for point in points {
        let mark = char::from_u32(point).unwrap();
        let body = format!("b{mark}a{mark}d");
        for mode in [MatchMode::Phrase, MatchMode::Compact] {
            assert!(matches("bad", &body, mode), "diacritic U+{point:04X}");
        }
    }
}

#[test]
fn every_specified_zero_width_bidi_and_bom_character_is_removed() {
    let points: Vec<u32> = [0x00AD, 0x2060, 0xFEFF]
        .into_iter()
        .chain(0x200C..=0x200F)
        .chain(0x202A..=0x202E)
        .chain(0x2066..=0x2069)
        .collect();
    for point in points {
        let control = char::from_u32(point).unwrap();
        let body = format!("b{control}a{control}d");
        for mode in [MatchMode::Phrase, MatchMode::Compact] {
            assert!(matches("bad", &body, mode), "control U+{point:04X}");
        }
    }
}

#[test]
fn phrase_boundaries_compact_substrings_and_literal_punctuation_are_deterministic() {
    assert!(matches("bad", "bad,", MatchMode::Phrase));
    assert!(!matches("bad", "badly", MatchMode::Phrase));
    assert!(matches("bad", "b a d", MatchMode::Compact));
    assert!(!matches("bad", "b a d", MatchMode::Phrase));
    assert!(matches("foo bar", "Foo, BAR!", MatchMode::Phrase));
    assert!(!matches("foo bar", "foobar", MatchMode::Phrase));
    assert!(matches("bad", "xxbadxx", MatchMode::Compact));
    assert!(!matches("a.*b", "axxb", MatchMode::Compact));
}

#[test]
fn thousand_character_inputs_are_bounded_and_keep_phrase_boundaries() {
    let body = format!("{}bad", "a".repeat(997));
    assert_eq!(body.chars().count(), 1000);
    assert!(matches("bad", &body, MatchMode::Compact));
    assert!(!matches("bad", &body, MatchMode::Phrase));
    assert!(!matches("bad", &"a".repeat(1000), MatchMode::Compact));
}

#[test]
fn benign_persian_near_matches_do_not_match_phrase_rules() {
    let benign = [
        "سلام، وقت بخیر",
        "این آگهی برای فروش است",
        "عنوان کتاب جدید",
        "گوهری زیبا",
        "بدترین انتخاب نیست",
    ];
    for body in benign {
        for phrase in ["عن", "گوه", "بد"] {
            assert!(
                !matches(phrase, body, MatchMode::Phrase),
                "{phrase}: {body}"
            );
        }
    }
}

#[test]
fn multiple_matches_preserve_rule_order() {
    let rules = [
        rule("first", "bad", MatchMode::Compact),
        rule("second", "ba", MatchMode::Compact),
        rule("third", "absent", MatchMode::Compact),
    ];
    let found = matching_rules("bad", &rules);
    let ids: Vec<&str> = found.iter().map(|rule| rule.public_id.as_str()).collect();
    assert_eq!(ids, ["first", "second"]);
}
