//! Deterministic Persian/Finglish literal normalization and comment matching.
//! No regular expressions, transliteration, fuzzy matching, or external service.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Phrase,
    Compact,
}

impl MatchMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "phrase" => Some(Self::Phrase),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Phrase => "phrase",
            Self::Compact => "compact",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedRule {
    pub phrase: String,
    pub normalized_phrase: String,
    pub dedupe_key: String,
    pub match_mode: MatchMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationRule {
    pub public_id: String,
    pub phrase: String,
    pub normalized_phrase: String,
    pub revision: u64,
    pub match_mode: MatchMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleClearance {
    pub public_id: String,
    pub revision: u64,
}

pub fn normalize_rule(
    phrase: &str,
    match_mode: MatchMode,
) -> Result<NormalizedRule, &'static str> {
    let phrase = phrase.trim();
    if phrase.is_empty() {
        return Err("required");
    }
    if phrase.chars().count() > 120 {
        return Err("too_long");
    }

    let phrase_form = normalize_phrase(phrase);
    let normalized_phrase = match match_mode {
        MatchMode::Phrase => phrase_form.clone(),
        MatchMode::Compact => normalize_compact(phrase),
    };
    if normalized_phrase
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .count()
        < 2
    {
        return Err("too_short");
    }

    Ok(NormalizedRule {
        phrase: phrase.to_owned(),
        normalized_phrase,
        dedupe_key: phrase_form,
        match_mode,
    })
}

pub fn matching_rules(body: &str, rules: &[ModerationRule]) -> Vec<ModerationRule> {
    let phrase = normalize_phrase(body);
    let compact = normalize_compact(body);
    rules
        .iter()
        .filter(|rule| {
            if rule.normalized_phrase.is_empty() {
                return false;
            }
            match rule.match_mode {
                MatchMode::Phrase => phrase_contains(&phrase, &rule.normalized_phrase),
                MatchMode::Compact => compact.contains(&rule.normalized_phrase),
            }
        })
        .cloned()
        .collect()
}

fn prepared(input: &str) -> impl Iterator<Item = char> + '_ {
    input.chars().filter_map(|ch| {
        let ch = match ch {
            '\u{064A}' | '\u{0649}' => '\u{06CC}',
            '\u{0643}' => '\u{06A9}',
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ => ch,
        };
        if is_removed(ch) {
            None
        } else {
            Some(ch)
        }
    })
}

fn is_removed(ch: char) -> bool {
    matches!(
        ch,
        '\u{064B}'..='\u{065F}'
            | '\u{0670}'
            | '\u{06D6}'..='\u{06ED}'
            | '\u{00AD}'
            | '\u{200C}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

fn normalize_phrase(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut previous = None;
    let mut separated = false;
    for ch in prepared(input) {
        if ch.is_alphanumeric() {
            if separated && !normalized.is_empty() {
                normalized.push(' ');
            }
            separated = false;
            if previous == Some(ch) {
                continue;
            }
            normalized.push(ch);
            previous = Some(ch);
        } else {
            separated = true;
            previous = None;
        }
    }
    normalized
}

fn normalize_compact(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut previous = None;
    for ch in prepared(input).filter(|ch| ch.is_alphanumeric()) {
        if previous == Some(ch) {
            continue;
        }
        normalized.push(ch);
        previous = Some(ch);
    }
    normalized
}

fn phrase_contains(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let at_word_start = start == 0 || haystack[..start].ends_with(' ');
        let at_word_end = end == haystack.len() || haystack[end..].starts_with(' ');
        at_word_start && at_word_end
    })
}
