//! Deterministic Persian/Finglish literal normalization and comment matching.
//! No regular expressions, transliteration, fuzzy matching, or external service.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Phrase,
    Compact,
}

impl MatchMode {
    pub fn parse(_value: &str) -> Option<Self> {
        todo!()
    }

    pub fn as_str(self) -> &'static str {
        todo!()
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
    _phrase: &str,
    _match_mode: MatchMode,
) -> Result<NormalizedRule, &'static str> {
    todo!()
}

pub fn matching_rules(_body: &str, _rules: &[ModerationRule]) -> Vec<ModerationRule> {
    todo!()
}
