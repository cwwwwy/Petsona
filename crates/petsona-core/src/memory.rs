//! Lightweight pet memory.
//!
//! This is deliberately not a chat transcript. It stores a small set of stable
//! facts, recent interaction events and greeting state in one JSON file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const MEMORY_VERSION: u32 = 1;
const MAX_EVENTS: usize = 200;
const MAX_FACTS: usize = 50;

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    pub id: String,
    pub key: String,
    pub value: String,
    pub confidence: f32,
    pub created_at: i64,
    pub updated_at: i64,
    /// Where this fact came from: `manual`, `conversation`, `import` or
    /// `compressed` (REQ-P06). Older files default to `manual`.
    #[serde(default = "default_fact_source")]
    pub source: String,
}

fn default_fact_source() -> String {
    FACT_SOURCE_MANUAL.to_string()
}

pub const FACT_SOURCE_MANUAL: &str = "manual";
pub const FACT_SOURCE_CONVERSATION: &str = "conversation";
pub const FACT_SOURCE_IMPORT: &str = "import";
pub const FACT_SOURCE_COMPRESSED: &str = "compressed";

/// True when the retention window has expired (0 days = keep forever).
pub fn retention_expired(created_at_ms: i64, now_ms: i64, days: u32) -> bool {
    if days == 0 {
        return false;
    }
    let window_ms = i64::from(days) * 24 * 60 * 60 * 1000;
    now_ms.saturating_sub(created_at_ms) > window_ms
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    AppStart,
    UserClick,
    UserMessage,
    PetGreeting,
    PetReaction,
    PetChanged,
    CodexStatus,
    IdleReturn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEvent {
    pub id: String,
    pub kind: EventKind,
    pub text: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCandidate {
    pub id: String,
    pub key: String,
    pub value: String,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemorySuggestion {
    pub key: String,
    pub value: String,
    pub confidence: f32,
    pub evidence_turn_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemorySuggestionDocument {
    #[serde(default)]
    candidates: Vec<MemorySuggestion>,
}

pub fn parse_memory_suggestions(text: &str) -> Result<Vec<MemorySuggestion>> {
    let document: MemorySuggestionDocument =
        serde_json::from_str(text.trim()).map_err(|error| {
            Error::config(format!("memory suggestions are not valid JSON: {error}"))
        })?;
    if document.candidates.len() > 12
        || document.candidates.iter().any(|candidate| {
            candidate.key.trim().is_empty()
                || candidate.key.chars().count() > 32
                || candidate.value.trim().is_empty()
                || candidate.value.chars().count() > 160
                || !candidate.confidence.is_finite()
                || !(0.0..=1.0).contains(&candidate.confidence)
                || candidate
                    .evidence_turn_ids
                    .iter()
                    .map(|id| id.trim())
                    .collect::<BTreeSet<_>>()
                    .len()
                    < 3
        })
    {
        return Err(Error::config(
            "memory suggestions contain invalid values or insufficient evidence",
        ));
    }
    Ok(document.candidates)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PersonaMemory {
    pub facts: Vec<Fact>,
    #[serde(default)]
    pub archived_facts: Vec<Fact>,
    #[serde(default)]
    pub candidates: Vec<MemoryCandidate>,
    #[serde(default)]
    pub learning_cursor: Option<String>,
    pub events: Vec<MemoryEvent>,
    pub last_seen_at: Option<i64>,
    pub last_greeting_at: Option<i64>,
    pub last_trigger: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct MemoryFile {
    pub version: u32,
    pub personas: BTreeMap<String, PersonaMemory>,
}

impl Default for MemoryFile {
    fn default() -> Self {
        Self {
            version: MEMORY_VERSION,
            personas: BTreeMap::new(),
        }
    }
}

/// File format written by [`MemoryStore::export_persona`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct MemoryExport {
    kind: String,
    version: u32,
    persona_id: String,
    #[serde(default)]
    facts: Vec<Fact>,
    #[serde(default)]
    archived_facts: Vec<Fact>,
    #[serde(default)]
    candidates: Vec<MemoryCandidate>,
    #[serde(default)]
    learning_cursor: Option<String>,
    #[serde(default)]
    events: Vec<MemoryEvent>,
}

pub const MEMORY_EXPORT_KIND: &str = "petsona.memory.persona";

#[derive(Debug, Clone, Default)]
pub struct GreetingContext {
    pub facts: Vec<Fact>,
    pub recent_events: Vec<MemoryEvent>,
    pub last_seen_at: Option<i64>,
    pub last_greeting_at: Option<i64>,
}

/// Extract an explicit, user-authored preference from a message.
///
/// This intentionally only accepts unambiguous first-person phrases. It is a
/// local, deterministic fallback for the native conversation flow, so a
/// provider outage cannot make the app invent a long-term preference.
pub fn extract_preference(text: &str) -> Option<(String, String, f32)> {
    // A question is never a statement about the user: "我叫什么？" must not be
    // remembered as 称呼 = 什么 (W19 feedback).
    if text.contains('？') || text.contains('?') {
        return None;
    }

    let text = text
        .trim()
        .trim_matches(|character: char| matches!(character, '。' | '！' | '，' | ',' | '.' | '!'));
    let chinese_patterns = [
        ("我不太喜欢", "不喜欢"),
        ("我不喜欢", "不喜欢"),
        ("我讨厌", "不喜欢"),
        ("我不想要", "不想要"),
        ("我更喜欢", "喜欢"),
        ("我比较喜欢", "喜欢"),
        ("我喜欢", "喜欢"),
        ("我更偏好", "偏好"),
        ("我偏好", "偏好"),
        ("我爱", "喜欢"),
        ("我想要", "想要"),
        ("以后请叫我", "称呼"),
        ("以后叫我", "称呼"),
        ("请叫我", "称呼"),
        ("我的名字是", "称呼"),
        ("我叫", "称呼"),
        ("我习惯", "习惯"),
        ("我平时", "习惯"),
        ("我通常", "习惯"),
        ("我经常", "习惯"),
        ("我每天", "习惯"),
    ];
    for (prefix, key) in chinese_patterns {
        if let Some(value) = text.strip_prefix(prefix).map(str::trim) {
            if let Some(value) = clean_preference_value(value) {
                return Some((key.to_string(), value, 0.9));
            }
        }
    }

    let lower = text.to_ascii_lowercase();
    let english_patterns = [
        ("i don't like ", "不喜欢"),
        ("i dislike ", "不喜欢"),
        ("i hate ", "不喜欢"),
        ("i prefer ", "偏好"),
        ("i like ", "喜欢"),
        ("i love ", "喜欢"),
        ("i usually ", "习惯"),
        ("i often ", "习惯"),
        ("i always ", "习惯"),
        ("call me ", "称呼"),
        ("my name is ", "称呼"),
    ];
    for (prefix, key) in english_patterns {
        if lower.strip_prefix(prefix).is_some() {
            if let Some(value) = clean_preference_value(&text[prefix.len()..]) {
                // Keep the user's original casing in the stored value.
                return Some((key.to_string(), value, 0.9));
            }
        }
    }
    None
}

/// Words that only ever appear in a question ("我叫**什么**"), plus dangling
/// filler particles. A captured value that starts with one of these is a
/// mis-parse, not a preference.
const INTERROGATIVE_PREFIXES: &[&str] = &[
    "什么",
    "啥",
    "哪",
    "谁",
    "多少",
    "几点",
    "怎么",
    "怎样",
    "如何",
    "为什么",
    "吗",
    "呢",
    "么",
    "what",
    "who",
    "which",
    "how",
    "why",
    "when",
    "where",
];

fn clean_preference_value(value: &str) -> Option<String> {
    let value = value
        .trim()
        .trim_matches(|character: char| {
            matches!(character, '。' | '！' | '，' | ',' | '.' | '!' | '?' | '？')
        })
        .trim();
    if value.is_empty() || value.chars().count() > 120 {
        return None;
    }

    let lower = value.to_ascii_lowercase();
    if INTERROGATIVE_PREFIXES
        .iter()
        .any(|prefix| value.starts_with(prefix) || lower.starts_with(prefix))
    {
        return None;
    }
    if value.ends_with(['吗', '呢', '么']) {
        return None;
    }

    Some(value.to_string())
}

impl GreetingContext {
    pub fn render(&self, now: i64) -> String {
        let mut sections = Vec::new();

        if !self.facts.is_empty() {
            let facts = self
                .facts
                .iter()
                .map(|fact| format!("- {}：{}", fact.key, fact.value))
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("【你记得关于用户的事情】\n{facts}"));
        }

        if !self.recent_events.is_empty() {
            let events = self
                .recent_events
                .iter()
                .map(|event| {
                    let text = event.text.as_deref().unwrap_or_else(|| event.kind.label());
                    format!("- {}（{}）", text, humanize_ago(now, event.created_at))
                })
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(format!("【最近互动】\n{events}"));
        }

        if let Some(last_seen) = self.last_seen_at {
            sections.push(format!("【上次见面】{}", humanize_ago(now, last_seen)));
        }

        if let Some(last_greeting) = self.last_greeting_at {
            sections.push(format!(
                "【上次主动问候】{}",
                humanize_ago(now, last_greeting)
            ));
        }

        sections.join("\n\n")
    }
}

impl EventKind {
    pub fn label(self) -> &'static str {
        match self {
            EventKind::AppStart => "应用启动",
            EventKind::UserClick => "用户点击了宠物",
            EventKind::UserMessage => "用户说了一句话",
            EventKind::PetGreeting => "宠物主动问候",
            EventKind::PetReaction => "宠物做出了回应",
            EventKind::PetChanged => "更换了宠物",
            EventKind::CodexStatus => "Codex 状态变化",
            EventKind::IdleReturn => "用户离开后回来",
        }
    }
}

pub struct PetMemory {
    path: PathBuf,
    inner: Mutex<MemoryFile>,
}

impl PetMemory {
    pub fn open(path: &Path) -> Result<Self> {
        let inner = if path.is_file() {
            let text = std::fs::read_to_string(path)?;
            serde_json::from_str(&text).map_err(|error| {
                Error::config(format!(
                    "cannot parse memory file {}: {error}",
                    path.display()
                ))
            })?
        } else {
            MemoryFile::default()
        };
        Ok(Self {
            path: path.to_path_buf(),
            inner: Mutex::new(inner),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn record_event(
        &self,
        persona_id: &str,
        kind: EventKind,
        text: Option<String>,
    ) -> Result<MemoryEvent> {
        let event = MemoryEvent {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            text,
            created_at: now_ms(),
        };
        let mut memory = self.inner.lock();
        let persona = memory.personas.entry(persona_id.to_string()).or_default();
        persona.last_seen_at = Some(event.created_at);
        persona.events.push(event.clone());
        trim_events(persona);
        self.save_locked(&memory)?;
        Ok(event)
    }

    pub fn remember_fact(
        &self,
        persona_id: &str,
        key: &str,
        value: &str,
        confidence: f32,
    ) -> Result<Fact> {
        self.remember_fact_from(persona_id, key, value, confidence, FACT_SOURCE_MANUAL)
    }

    /// Same as [`Self::remember_fact`] but records where the fact came from.
    pub fn remember_fact_from(
        &self,
        persona_id: &str,
        key: &str,
        value: &str,
        confidence: f32,
        source: &str,
    ) -> Result<Fact> {
        let now = now_ms();
        let mut memory = self.inner.lock();
        let persona = memory.personas.entry(persona_id.to_string()).or_default();
        let active_index = persona
            .facts
            .iter_mut()
            .position(|fact| fact_matches(fact, key, value));
        let archived_index = if active_index.is_none() {
            persona
                .archived_facts
                .iter_mut()
                .position(|fact| fact_matches(fact, key, value))
        } else {
            None
        };
        let fact = if let Some(index) = active_index {
            let fact = &mut persona.facts[index];
            fact.value = value.to_string();
            fact.confidence = confidence;
            fact.updated_at = now;
            fact.source = source.to_string();
            fact.clone()
        } else if let Some(index) = archived_index {
            let fact = &mut persona.archived_facts[index];
            fact.value = value.to_string();
            fact.confidence = confidence;
            fact.updated_at = now;
            fact.source = source.to_string();
            fact.clone()
        } else {
            let fact = Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key: key.to_string(),
                value: value.to_string(),
                confidence,
                created_at: now,
                updated_at: now,
                source: source.to_string(),
            };
            persona.facts.push(fact.clone());
            fact
        };
        persona
            .facts
            .sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        persona.facts.truncate(MAX_FACTS);
        self.save_locked(&memory)?;
        Ok(fact)
    }

    /// Store an explicitly stated preference without silently replacing facts
    /// the user edited by hand. Multi-valued keys such as "喜欢" can coexist;
    /// a new form of address replaces an older conversation-derived form, and
    /// explicit positive/negative statements retract their conversation-derived
    /// opposite for the same value.
    pub fn remember_explicit_preference(
        &self,
        persona_id: &str,
        key: &str,
        value: &str,
        confidence: f32,
        evidence: &str,
    ) -> Result<bool> {
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Ok(false);
        }

        let mut memory = self.inner.lock();
        let persona = memory.personas.entry(persona_id.to_string()).or_default();
        let exact_facts = persona.facts.iter().chain(persona.archived_facts.iter());
        if exact_facts
            .clone()
            .any(|fact| fact_matches(fact, key, value) && fact.source == FACT_SOURCE_MANUAL)
        {
            return Ok(false);
        }

        let has_manual_conflict = persona
            .facts
            .iter()
            .chain(persona.archived_facts.iter())
            .any(|fact| {
                fact.source == FACT_SOURCE_MANUAL && explicit_preference_conflicts(fact, key, value)
            });
        if has_manual_conflict {
            let evidence = evidence.trim();
            if evidence.is_empty() {
                return Ok(false);
            }
            if let Some(candidate) = persona.candidates.iter_mut().find(|candidate| {
                candidate.key.eq_ignore_ascii_case(key)
                    && candidate.value.eq_ignore_ascii_case(value)
            }) {
                if candidate.status != "pending" {
                    return Ok(false);
                }
                if !candidate.evidence.iter().any(|item| item == evidence) {
                    candidate.evidence.push(evidence.to_string());
                    candidate.evidence.truncate(12);
                }
                candidate.confidence = candidate.confidence.max(confidence.clamp(0.0, 1.0));
                candidate.updated_at = now_ms();
            } else {
                persona.candidates.push(MemoryCandidate {
                    id: uuid::Uuid::new_v4().to_string(),
                    key: key.to_string(),
                    value: value.to_string(),
                    confidence: confidence.clamp(0.0, 1.0),
                    evidence: vec![evidence.to_string()],
                    status: "pending".to_string(),
                    created_at: now_ms(),
                    updated_at: now_ms(),
                });
                if persona.candidates.len() > 100 {
                    persona.candidates.drain(0..persona.candidates.len() - 100);
                }
            }
            self.save_locked(&memory)?;
            return Ok(false);
        }

        persona.facts.retain(|fact| {
            !(fact.source == FACT_SOURCE_CONVERSATION
                && explicit_preference_conflicts(fact, key, value))
        });
        persona.archived_facts.retain(|fact| {
            !(fact.source == FACT_SOURCE_CONVERSATION
                && explicit_preference_conflicts(fact, key, value))
        });

        let now = now_ms();
        if let Some(fact) = persona
            .facts
            .iter_mut()
            .find(|fact| fact_matches(fact, key, value))
        {
            fact.confidence = confidence.clamp(0.0, 1.0);
            fact.updated_at = now;
            fact.source = FACT_SOURCE_CONVERSATION.to_string();
        } else if let Some(fact) = persona
            .archived_facts
            .iter_mut()
            .find(|fact| fact_matches(fact, key, value))
        {
            fact.confidence = confidence.clamp(0.0, 1.0);
            fact.updated_at = now;
            fact.source = FACT_SOURCE_CONVERSATION.to_string();
        } else {
            let fact = Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key: key.to_string(),
                value: value.to_string(),
                confidence: confidence.clamp(0.0, 1.0),
                created_at: now,
                updated_at: now,
                source: FACT_SOURCE_CONVERSATION.to_string(),
            };
            persona.facts.push(fact);
        }
        persona
            .facts
            .sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        persona.facts.truncate(MAX_FACTS);
        self.save_locked(&memory)?;
        Ok(true)
    }

    /// Add an inferred habit for confirmation. Rejected or confirmed
    /// key/value pairs are not re-proposed; repeated proposals merge evidence.
    pub fn propose_candidate(
        &self,
        persona_id: &str,
        key: &str,
        value: &str,
        confidence: f32,
        evidence: &[String],
    ) -> Result<Option<MemoryCandidate>> {
        let key = key.trim();
        let value = value.trim();
        let distinct_evidence = evidence
            .iter()
            .map(|item| item.trim())
            .collect::<BTreeSet<_>>();
        if key.is_empty() || value.is_empty() || distinct_evidence.len() < 3 {
            return Ok(None);
        }
        let now = now_ms();
        let mut memory = self.inner.lock();
        let persona = memory.personas.entry(persona_id.to_string()).or_default();
        if persona
            .facts
            .iter()
            .chain(persona.archived_facts.iter())
            .any(|fact| {
                fact.key.eq_ignore_ascii_case(key) && fact.value.eq_ignore_ascii_case(value)
            })
        {
            return Ok(None);
        }
        if let Some(candidate) = persona.candidates.iter_mut().find(|candidate| {
            candidate.key.eq_ignore_ascii_case(key) && candidate.value.eq_ignore_ascii_case(value)
        }) {
            if candidate.status != "pending" {
                return Ok(None);
            }
            for item in evidence
                .iter()
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
            {
                if !candidate.evidence.iter().any(|old| old == item) {
                    candidate.evidence.push(item.to_string());
                }
            }
            candidate.confidence = candidate.confidence.max(confidence.clamp(0.0, 1.0));
            candidate.updated_at = now;
            let result = candidate.clone();
            self.save_locked(&memory)?;
            return Ok(Some(result));
        }
        let candidate = MemoryCandidate {
            id: uuid::Uuid::new_v4().to_string(),
            key: key.to_string(),
            value: value.to_string(),
            confidence: confidence.clamp(0.0, 1.0),
            evidence: evidence
                .iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .take(12)
                .collect(),
            status: "pending".to_string(),
            created_at: now,
            updated_at: now,
        };
        persona.candidates.push(candidate.clone());
        if persona.candidates.len() > 100 {
            persona.candidates.drain(0..persona.candidates.len() - 100);
        }
        self.save_locked(&memory)?;
        Ok(Some(candidate))
    }

    pub fn review_candidate(
        &self,
        persona_id: &str,
        candidate_id: &str,
        accept: bool,
    ) -> Result<Option<Fact>> {
        let mut memory = self.inner.lock();
        let Some(persona) = memory.personas.get_mut(persona_id) else {
            return Ok(None);
        };
        let Some(index) = persona
            .candidates
            .iter()
            .position(|candidate| candidate.id == candidate_id && candidate.status == "pending")
        else {
            return Ok(None);
        };
        let now = now_ms();
        let candidate = &mut persona.candidates[index];
        candidate.status = if accept { "confirmed" } else { "rejected" }.to_string();
        candidate.updated_at = now;
        let fact = if accept {
            let candidate = candidate.clone();
            let existing = persona.facts.iter_mut().find(|fact| {
                fact.key.eq_ignore_ascii_case(&candidate.key)
                    && fact.value.eq_ignore_ascii_case(&candidate.value)
            });
            let fact = match existing {
                Some(fact) => {
                    fact.confidence = candidate.confidence;
                    fact.updated_at = now;
                    fact.source = FACT_SOURCE_CONVERSATION.to_string();
                    fact.clone()
                }
                None => Fact {
                    id: uuid::Uuid::new_v4().to_string(),
                    key: candidate.key,
                    value: candidate.value,
                    confidence: candidate.confidence,
                    created_at: now,
                    updated_at: now,
                    source: FACT_SOURCE_CONVERSATION.to_string(),
                },
            };
            if !persona.facts.iter().any(|existing| existing.id == fact.id) {
                persona.facts.push(fact.clone());
            }
            Some(fact)
        } else {
            None
        };
        persona
            .facts
            .sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        if persona.facts.len() > MAX_FACTS {
            persona.facts.truncate(MAX_FACTS);
        }
        self.save_locked(&memory)?;
        Ok(fact)
    }

    pub fn learning_cursor(&self, persona_id: &str) -> Option<String> {
        self.inner
            .lock()
            .personas
            .get(persona_id)
            .and_then(|persona| persona.learning_cursor.clone())
    }

    pub fn mark_learning_cursor(&self, persona_id: &str, turn_id: &str) -> Result<()> {
        let mut memory = self.inner.lock();
        let persona = memory.personas.entry(persona_id.to_string()).or_default();
        persona.learning_cursor = Some(turn_id.to_string());
        self.save_locked(&memory)
    }

    pub fn forget_fact(&self, persona_id: &str, fact_id: &str) -> Result<bool> {
        let mut memory = self.inner.lock();
        let Some(persona) = memory.personas.get_mut(persona_id) else {
            return Ok(false);
        };
        let before = persona.facts.len();
        persona.facts.retain(|fact| fact.id != fact_id);
        let archived_before = persona.archived_facts.len();
        persona.archived_facts.retain(|fact| fact.id != fact_id);
        let removed =
            persona.facts.len() != before || persona.archived_facts.len() != archived_before;
        if removed {
            self.save_locked(&memory)?;
        }
        Ok(removed)
    }

    /// Edit an existing fact in place (manual correction from the settings
    /// page). `created_at` is preserved, `updated_at` moves (REQ-S15).
    pub fn update_fact(
        &self,
        persona_id: &str,
        fact_id: &str,
        key: &str,
        value: &str,
        confidence: f32,
    ) -> Result<Option<Fact>> {
        let now = now_ms();
        let mut memory = self.inner.lock();
        let Some(persona) = memory.personas.get_mut(persona_id) else {
            return Ok(None);
        };
        let fact = persona
            .facts
            .iter_mut()
            .chain(persona.archived_facts.iter_mut())
            .find(|fact| fact.id == fact_id);
        let Some(fact) = fact else { return Ok(None) };
        fact.key = key.to_string();
        fact.value = value.to_string();
        fact.confidence = confidence.clamp(0.0, 1.0);
        fact.updated_at = now;
        fact.source = FACT_SOURCE_MANUAL.to_string();
        let updated = fact.clone();
        persona
            .facts
            .sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        self.save_locked(&memory)?;
        Ok(Some(updated))
    }

    /// Drop events older than the retention window (REQ-P05). Returns how many
    /// events were removed; `days == 0` keeps everything.
    pub fn prune_events(&self, persona_id: &str, days: u32) -> Result<usize> {
        if days == 0 {
            return Ok(0);
        }
        let now = now_ms();
        let mut memory = self.inner.lock();
        let Some(persona) = memory.personas.get_mut(persona_id) else {
            return Ok(0);
        };
        let before = persona.events.len();
        persona
            .events
            .retain(|event| !retention_expired(event.created_at, now, days));
        let removed = before - persona.events.len();
        if removed > 0 {
            self.save_locked(&memory)?;
        }
        Ok(removed)
    }

    /// Keep the `keep` most recently updated facts; everything older is folded
    /// into one deterministic 「画像」 fact so nothing is silently dropped
    /// (REQ-P05). Returns how many facts were merged away.
    pub fn compress_facts(&self, persona_id: &str, keep: usize) -> Result<usize> {
        let mut memory = self.inner.lock();
        let Some(persona) = memory.personas.get_mut(persona_id) else {
            return Ok(0);
        };
        if persona.facts.len() <= keep.max(1) {
            return Ok(0);
        }

        let keep = keep.max(1);
        let mut sorted = persona.facts.clone();
        sorted.sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        let (recent, old) = sorted.split_at(keep);
        let new_archived = old
            .iter()
            .filter(|fact| !(fact.key == "画像" && fact.source == FACT_SOURCE_COMPRESSED))
            .filter(|fact| !persona.archived_facts.iter().any(|old| old.id == fact.id))
            .cloned()
            .collect::<Vec<_>>();
        let merged_count = new_archived.len();
        persona.archived_facts.extend(new_archived);
        persona
            .archived_facts
            .sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        let merged = persona
            .archived_facts
            .iter()
            .take(30)
            .map(|fact| format!("{}：{}（来源：{}）", fact.key, fact.value, fact.source))
            .collect::<Vec<_>>()
            .join("；")
            .chars()
            .take(1800)
            .collect::<String>();

        let previous_profile = persona
            .facts
            .iter()
            .find(|fact| fact.key == "画像" && fact.source == FACT_SOURCE_COMPRESSED)
            .cloned();
        let now = now_ms();
        let profile = match previous_profile {
            Some(mut profile) => {
                profile.value = merged;
                profile.updated_at = now;
                profile
            }
            None => Fact {
                id: uuid::Uuid::new_v4().to_string(),
                key: "画像".to_string(),
                value: merged,
                confidence: 0.6,
                created_at: now,
                updated_at: now,
                source: FACT_SOURCE_COMPRESSED.to_string(),
            },
        };

        // The previous profile (if any) is replaced by the merged one instead of
        // being kept next to it.
        persona.facts = recent
            .iter()
            .filter(|fact| !(fact.key == "画像" && fact.source == FACT_SOURCE_COMPRESSED))
            .cloned()
            .collect();
        persona.facts.push(profile);
        persona
            .facts
            .sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        persona.facts.truncate(keep.saturating_add(1));
        self.save_locked(&memory)?;
        Ok(merged_count)
    }

    /// Remove every fact of a persona but keep its events (and vice versa with
    /// [`Self::clear_events`]). Returns how many entries were dropped.
    pub fn clear_facts(&self, persona_id: &str) -> Result<usize> {
        let mut memory = self.inner.lock();
        let Some(persona) = memory.personas.get_mut(persona_id) else {
            return Ok(0);
        };
        let removed = persona.facts.len() + persona.archived_facts.len() + persona.candidates.len();
        persona.facts.clear();
        persona.archived_facts.clear();
        persona.candidates.clear();
        persona.learning_cursor = None;
        if removed > 0 {
            self.save_locked(&memory)?;
        }
        Ok(removed)
    }

    pub fn clear_events(&self, persona_id: &str) -> Result<usize> {
        let mut memory = self.inner.lock();
        let Some(persona) = memory.personas.get_mut(persona_id) else {
            return Ok(0);
        };
        let removed = persona.events.len();
        persona.events.clear();
        if removed > 0 {
            self.save_locked(&memory)?;
        }
        Ok(removed)
    }

    /// Write one persona's facts and events to a portable JSON file.
    pub fn export_persona(&self, persona_id: &str, path: &Path) -> Result<PersonaMemory> {
        let memory = self.inner.lock();
        let exported = memory.personas.get(persona_id).cloned().unwrap_or_default();
        let bundle = MemoryExport {
            kind: MEMORY_EXPORT_KIND.to_string(),
            version: MEMORY_VERSION,
            persona_id: persona_id.to_string(),
            facts: exported.facts.clone(),
            archived_facts: exported.archived_facts.clone(),
            candidates: exported.candidates.clone(),
            learning_cursor: exported.learning_cursor.clone(),
            events: exported.events.clone(),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&bundle)?)?;
        Ok(exported)
    }

    /// Replace one persona's facts and events with an exported file. The
    /// conversation bookkeeping (last seen / last greeting) is preserved.
    pub fn import_persona(&self, persona_id: &str, path: &Path) -> Result<(usize, usize)> {
        let raw = std::fs::read_to_string(path)?;
        let bundle: MemoryExport = serde_json::from_str(&raw)
            .map_err(|error| Error::config(format!("不是有效的记忆导出文件：{error}")))?;
        if bundle.kind != MEMORY_EXPORT_KIND {
            return Err(Error::config("记忆导出文件的 kind 字段不匹配"));
        }

        let facts = bundle.facts.len();
        let events = bundle.events.len();
        let mut memory = self.inner.lock();
        let persona = memory.personas.entry(persona_id.to_string()).or_default();
        persona.facts = bundle.facts;
        persona.archived_facts = bundle.archived_facts;
        persona.candidates = bundle.candidates;
        persona.learning_cursor = bundle.learning_cursor;
        persona.events = bundle.events;
        trim_events(persona);
        persona
            .facts
            .sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        persona.facts.truncate(MAX_FACTS);
        self.save_locked(&memory)?;
        Ok((facts, events))
    }

    pub fn list_facts(&self, persona_id: &str) -> Vec<Fact> {
        self.inner
            .lock()
            .personas
            .get(persona_id)
            .map(|persona| persona.facts.clone())
            .unwrap_or_default()
    }

    /// Return the persisted memory projection for a persona without exposing
    /// the internal mutex or allowing callers to mutate it.
    pub fn persona_snapshot(&self, persona_id: &str) -> PersonaMemory {
        self.inner
            .lock()
            .personas
            .get(persona_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn recent_events(&self, persona_id: &str, limit: usize) -> Vec<MemoryEvent> {
        let memory = self.inner.lock();
        let mut events = memory
            .personas
            .get(persona_id)
            .map(|persona| persona.events.clone())
            .unwrap_or_default();
        if events.len() > limit {
            events.drain(0..events.len() - limit);
        }
        events
    }

    pub fn mark_seen(&self, persona_id: &str) -> Result<()> {
        let mut memory = self.inner.lock();
        let persona = memory.personas.entry(persona_id.to_string()).or_default();
        persona.last_seen_at = Some(now_ms());
        self.save_locked(&memory)
    }

    pub fn mark_greeted(&self, persona_id: &str, trigger: &str, text: &str) -> Result<()> {
        let now = now_ms();
        let mut memory = self.inner.lock();
        let persona = memory.personas.entry(persona_id.to_string()).or_default();
        persona.last_greeting_at = Some(now);
        persona.last_trigger = Some(trigger.to_string());
        persona.events.push(MemoryEvent {
            id: uuid::Uuid::new_v4().to_string(),
            kind: EventKind::PetGreeting,
            text: Some(text.to_string()),
            created_at: now,
        });
        trim_events(persona);
        self.save_locked(&memory)
    }

    pub fn clear_persona(&self, persona_id: &str) -> Result<()> {
        let mut memory = self.inner.lock();
        memory.personas.remove(persona_id);
        self.save_locked(&memory)
    }

    pub fn build_greeting_context(
        &self,
        persona_id: &str,
        recent_events: usize,
        fact_limit: usize,
    ) -> GreetingContext {
        let memory = self.inner.lock();
        let Some(persona) = memory.personas.get(persona_id) else {
            return GreetingContext::default();
        };
        let mut facts = persona.facts.clone();
        facts.sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        facts.truncate(fact_limit);

        let mut events = persona.events.clone();
        if events.len() > recent_events {
            events.drain(0..events.len() - recent_events);
        }

        GreetingContext {
            facts,
            recent_events: events,
            last_seen_at: persona.last_seen_at,
            last_greeting_at: persona.last_greeting_at,
        }
    }

    fn save_locked(&self, memory: &MemoryFile) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_string_pretty(memory)?)?;
        std::fs::rename(temp, &self.path).map_err(Error::from)
    }
}

fn fact_matches(fact: &Fact, key: &str, value: &str) -> bool {
    fact.key.eq_ignore_ascii_case(key) && fact.value.trim().eq_ignore_ascii_case(value.trim())
}

fn explicit_preference_conflicts(existing: &Fact, key: &str, value: &str) -> bool {
    if key.eq_ignore_ascii_case("称呼")
        && existing.key.eq_ignore_ascii_case(key)
        && !existing.value.trim().eq_ignore_ascii_case(value.trim())
    {
        return true;
    }
    match opposite_preference_key(key) {
        Some(opposite) => {
            existing.key.eq_ignore_ascii_case(opposite)
                && existing.value.trim().eq_ignore_ascii_case(value.trim())
        }
        None => false,
    }
}

fn opposite_preference_key(key: &str) -> Option<&'static str> {
    if key.eq_ignore_ascii_case("喜欢") {
        Some("不喜欢")
    } else if key.eq_ignore_ascii_case("不喜欢") {
        Some("喜欢")
    } else if key.eq_ignore_ascii_case("想要") {
        Some("不想要")
    } else if key.eq_ignore_ascii_case("不想要") {
        Some("想要")
    } else {
        None
    }
}

fn trim_events(persona: &mut PersonaMemory) {
    if persona.events.len() > MAX_EVENTS {
        persona.events.drain(0..persona.events.len() - MAX_EVENTS);
    }
}

fn humanize_ago(now: i64, then: i64) -> String {
    let seconds = ((now - then).max(0) / 1000) as u64;
    if seconds < 60 {
        "刚刚".to_string()
    } else if seconds < 3600 {
        format!("{} 分钟前", seconds / 60)
    } else if seconds < 86_400 {
        format!("{} 小时前", seconds / 3600)
    } else {
        format!("{} 天前", seconds / 86_400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_upsert_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let memory = PetMemory::open(&dir.path().join("memory.json")).unwrap();
        memory
            .remember_fact("default", "咖啡", "美式", 1.0)
            .unwrap();
        memory
            .remember_fact("default", "咖啡", "拿铁", 0.9)
            .unwrap();
        memory
            .remember_fact("default", "咖啡", "美式", 0.8)
            .unwrap();
        let facts = memory.list_facts("default");
        assert_eq!(facts.len(), 2);
        assert!(facts.iter().any(|fact| fact.value == "美式"));
        assert!(facts.iter().any(|fact| fact.value == "拿铁"));
        memory.forget_fact("default", &facts[0].id).unwrap();
        assert_eq!(memory.list_facts("default").len(), 1);
        memory.clear_facts("default").unwrap();
        assert!(memory.list_facts("default").is_empty());
    }

    #[test]
    fn invalid_memory_is_reported_without_replacing_the_original_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        std::fs::write(&path, b"invalid json").unwrap();

        assert!(PetMemory::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid json");
    }

    #[test]
    fn explicit_preferences_coexist_but_retract_the_opposite_for_the_same_value() {
        let dir = tempfile::tempdir().unwrap();
        let memory = PetMemory::open(&dir.path().join("memory.json")).unwrap();
        memory
            .remember_explicit_preference("default", "喜欢", "咖啡", 0.9, "我喜欢咖啡")
            .unwrap();
        memory
            .remember_explicit_preference("default", "喜欢", "茶", 0.9, "我喜欢茶")
            .unwrap();
        memory
            .remember_explicit_preference("default", "不喜欢", "咖啡", 0.9, "我不喜欢咖啡")
            .unwrap();

        let facts = memory.list_facts("default");
        assert!(facts
            .iter()
            .any(|fact| fact.key == "喜欢" && fact.value == "茶"));
        assert!(facts
            .iter()
            .any(|fact| fact.key == "不喜欢" && fact.value == "咖啡"));
        assert!(!facts
            .iter()
            .any(|fact| fact.key == "喜欢" && fact.value == "咖啡"));
    }

    #[test]
    fn explicit_correction_to_a_manual_fact_waits_for_user_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let memory = PetMemory::open(&dir.path().join("memory.json")).unwrap();
        memory
            .remember_fact("default", "称呼", "小宁", 1.0)
            .unwrap();

        let recorded = memory
            .remember_explicit_preference("default", "称呼", "宁宁", 0.9, "请叫我宁宁")
            .unwrap();

        assert!(!recorded);
        let snapshot = memory.persona_snapshot("default");
        assert_eq!(snapshot.facts.len(), 1);
        assert_eq!(snapshot.facts[0].value, "小宁");
        assert_eq!(snapshot.candidates.len(), 1);
        assert_eq!(snapshot.candidates[0].value, "宁宁");
        assert_eq!(snapshot.candidates[0].status, "pending");
        assert_eq!(snapshot.candidates[0].evidence, vec!["请叫我宁宁"]);
    }

    #[test]
    fn explicit_conversation_correction_replaces_an_old_form_of_address() {
        let dir = tempfile::tempdir().unwrap();
        let memory = PetMemory::open(&dir.path().join("memory.json")).unwrap();
        memory
            .remember_explicit_preference("default", "称呼", "小宁", 0.9, "请叫我小宁")
            .unwrap();
        memory
            .remember_explicit_preference("default", "称呼", "宁宁", 0.9, "请叫我宁宁")
            .unwrap();

        let facts = memory.list_facts("default");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].value, "宁宁");
    }

    #[test]
    fn habit_suggestions_require_three_distinct_user_messages() {
        let good = r#"{"candidates":[{"key":"习惯","value":"下午散步","confidence":0.8,"evidenceTurnIds":["u1","u2","u3"]}]}"#;
        let parsed = parse_memory_suggestions(good).unwrap();
        assert_eq!(parsed.len(), 1);
        let repeated_id = r#"{"candidates":[{"key":"习惯","value":"下午散步","confidence":0.8,"evidenceTurnIds":["u1","u1","u2"]}]}"#;
        assert!(parse_memory_suggestions(repeated_id).is_err());
        assert!(parse_memory_suggestions("not json").is_err());
    }

    #[test]
    fn events_and_context_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let memory = PetMemory::open(&dir.path().join("memory.json")).unwrap();
        memory
            .record_event("default", EventKind::AppStart, Some("启动".into()))
            .unwrap();
        memory
            .record_event("default", EventKind::UserClick, None)
            .unwrap();
        let context = memory.build_greeting_context("default", 5, 20);
        assert_eq!(context.recent_events.len(), 2);
        assert!(context.render(now_ms()).contains("最近互动"));
    }

    #[test]
    fn edit_and_scoped_clears_keep_the_other_half() {
        let dir = tempfile::tempdir().unwrap();
        let memory = PetMemory::open(&dir.path().join("memory.json")).unwrap();
        memory
            .remember_fact("default", "咖啡", "美式", 1.0)
            .unwrap();
        memory
            .record_event("default", EventKind::UserClick, None)
            .unwrap();

        let fact = memory.list_facts("default").remove(0);
        let updated = memory
            .update_fact("default", &fact.id, "咖啡", "拿铁", 0.5)
            .unwrap()
            .expect("fact is updated in place");
        assert_eq!(updated.id, fact.id, "editing keeps the fact identity");
        assert_eq!(updated.created_at, fact.created_at);
        assert_eq!(updated.value, "拿铁");

        // Clearing facts keeps events and vice versa (REQ-S15).
        assert_eq!(memory.clear_facts("default").unwrap(), 1);
        assert!(memory.list_facts("default").is_empty());
        assert_eq!(memory.recent_events("default", 10).len(), 1);
        assert_eq!(memory.clear_events("default").unwrap(), 1);
        assert!(memory.recent_events("default", 10).is_empty());

        // Editing a fact that no longer exists reports "none" instead of failing.
        assert!(memory
            .update_fact("default", &fact.id, "咖啡", "摩卡", 0.5)
            .unwrap()
            .is_none());
    }

    #[test]
    fn retention_prunes_only_expired_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let memory = PetMemory::open(&path).unwrap();
        memory
            .record_event("default", EventKind::AppStart, Some("新".into()))
            .unwrap();
        memory
            .record_event("default", EventKind::UserClick, Some("旧".into()))
            .unwrap();

        // Backdate the "旧" event by hand: 10 days old, retention of 3 days.
        let mut file: MemoryFile =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let persona = file.personas.get_mut("default").unwrap();
        let old = now_ms() - 10 * 24 * 60 * 60 * 1000;
        for event in persona.events.iter_mut() {
            if event.text.as_deref() == Some("旧") {
                event.created_at = old;
            }
        }
        std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();

        let reopened = PetMemory::open(&path).unwrap();
        assert_eq!(reopened.prune_events("default", 3).unwrap(), 1);
        let events = reopened.recent_events("default", 10);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].text.as_deref(), Some("新"));

        // 0 days = keep everything.
        assert_eq!(reopened.prune_events("default", 0).unwrap(), 0);
        assert_eq!(reopened.recent_events("default", 10).len(), 1);
    }

    #[test]
    fn compression_folds_old_facts_into_a_profile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.json");
        let memory = PetMemory::open(&path).unwrap();
        for (key, value) in [("咖啡", "美式"), ("茶", "乌龙"), ("音乐", "爵士")] {
            memory.remember_fact("default", key, value, 0.9).unwrap();
        }

        // Explicit timestamps: 咖啡 is the oldest, 音乐 the newest.
        let now = now_ms();
        let mut file: MemoryFile =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let persona = file.personas.get_mut("default").unwrap();
        for fact in persona.facts.iter_mut() {
            fact.updated_at = match fact.key.as_str() {
                "咖啡" => now - 3 * 24 * 60 * 60 * 1000,
                "茶" => now - 2 * 24 * 60 * 60 * 1000,
                _ => now - 24 * 60 * 60 * 1000,
            };
        }
        std::fs::write(&path, serde_json::to_string(&file).unwrap()).unwrap();
        let memory = PetMemory::open(&path).unwrap();

        assert_eq!(memory.compress_facts("default", 2).unwrap(), 1);
        let facts = memory.list_facts("default");
        assert_eq!(facts.len(), 3, "2 kept + 1 profile");
        let profile = facts.iter().find(|fact| fact.key == "画像").unwrap();
        assert_eq!(profile.source, FACT_SOURCE_COMPRESSED);
        assert!(profile.value.contains("咖啡：美式"), "{}", profile.value);
        assert!(!facts.iter().any(|fact| fact.key == "咖啡"));
        let snapshot = memory.persona_snapshot("default");
        assert_eq!(snapshot.archived_facts.len(), 1);
        assert_eq!(snapshot.archived_facts[0].key, "咖啡");

        // Compressing again folds into the same profile instead of duplicating it.
        assert_eq!(memory.compress_facts("default", 2).unwrap(), 1);
        let facts = memory.list_facts("default");
        assert_eq!(facts.iter().filter(|fact| fact.key == "画像").count(), 1);
        assert!(!memory.persona_snapshot("default").archived_facts.is_empty());
    }

    #[test]
    fn repeated_habit_requires_confirmation_and_rejection_prevents_reproposal() {
        let dir = tempfile::tempdir().unwrap();
        let memory = PetMemory::open(&dir.path().join("memory.json")).unwrap();
        let evidence = vec![
            "u1: 每天下午我都会散步".to_string(),
            "u2: 下午继续出去散步了".to_string(),
            "u3: 我今天也按习惯散步".to_string(),
        ];
        let candidate = memory
            .propose_candidate("pet-a", "习惯", "下午散步", 0.85, &evidence)
            .unwrap()
            .unwrap();
        assert_eq!(candidate.status, "pending");
        assert!(memory.list_facts("pet-a").is_empty());

        let fact = memory
            .review_candidate("pet-a", &candidate.id, true)
            .unwrap()
            .unwrap();
        assert_eq!(fact.key, "习惯");
        assert_eq!(fact.value, "下午散步");
        assert_eq!(fact.source, FACT_SOURCE_CONVERSATION);
        assert!(memory
            .list_facts("pet-a")
            .iter()
            .any(|item| item.id == fact.id));
        assert!(memory
            .propose_candidate("pet-a", "习惯", "下午散步", 0.9, &evidence)
            .unwrap()
            .is_none());

        let rejected = memory
            .propose_candidate("pet-b", "习惯", "上午喝茶", 0.8, &evidence)
            .unwrap()
            .unwrap();
        assert!(memory
            .review_candidate("pet-b", &rejected.id, false)
            .unwrap()
            .is_none());
        assert!(memory
            .propose_candidate("pet-b", "习惯", "上午喝茶", 0.9, &evidence)
            .unwrap()
            .is_none());
        assert!(memory.list_facts("pet-b").is_empty());
    }

    #[test]
    fn export_then_import_round_trips_one_persona() {
        let dir = tempfile::tempdir().unwrap();
        let memory = PetMemory::open(&dir.path().join("memory.json")).unwrap();
        memory
            .remember_fact("default", "咖啡", "美式", 1.0)
            .unwrap();
        memory
            .record_event("default", EventKind::AppStart, Some("启动".into()))
            .unwrap();
        memory.remember_fact("other", "茶", "乌龙", 0.8).unwrap();

        let export = dir.path().join("export.json");
        memory.export_persona("default", &export).unwrap();

        // Wipe, then import: facts and events come back, other personas untouched.
        memory.clear_persona("default").unwrap();
        assert!(memory.list_facts("default").is_empty());
        let (facts, events) = memory.import_persona("default", &export).unwrap();
        assert_eq!((facts, events), (1, 1));
        assert_eq!(memory.list_facts("default")[0].value, "美式");
        assert_eq!(memory.recent_events("default", 10).len(), 1);
        assert_eq!(memory.list_facts("other")[0].value, "乌龙");

        // A foreign file is rejected with a readable error.
        let foreign = dir.path().join("foreign.json");
        std::fs::write(
            &foreign,
            "{\"kind\":\"other\",\"version\":1,\"personaId\":\"x\"}",
        )
        .unwrap();
        assert!(memory.import_persona("default", &foreign).is_err());
    }

    #[test]
    fn questions_are_never_stored_as_preferences() {
        // Regression from the W19 acceptance round.
        assert_eq!(extract_preference("我叫什么？"), None);
        assert_eq!(extract_preference("我叫什么"), None);
        assert_eq!(extract_preference("我的名字是什么"), None);
        assert_eq!(extract_preference("我喜欢什么"), None);
        assert_eq!(extract_preference("请叫我什么？"), None);
        assert_eq!(extract_preference("What is my name?"), None);
        assert_eq!(extract_preference("我叫小明吗"), None);
        // Real statements still work.
        assert_eq!(
            extract_preference("我叫小明"),
            Some(("称呼".to_string(), "小明".to_string(), 0.9))
        );
    }

    #[test]
    fn extracts_only_explicit_preferences() {
        assert_eq!(
            extract_preference("我喜欢安静的音乐。"),
            Some(("喜欢".to_string(), "安静的音乐".to_string(), 0.9))
        );
        assert_eq!(
            extract_preference("Call me Book!"),
            Some(("称呼".to_string(), "Book".to_string(), 0.9))
        );
        assert_eq!(
            extract_preference("我更喜欢乌龙茶"),
            Some(("喜欢".to_string(), "乌龙茶".to_string(), 0.9))
        );
        assert_eq!(
            extract_preference("我平时早上散步"),
            Some(("习惯".to_string(), "早上散步".to_string(), 0.9))
        );
        assert!(extract_preference("今天感觉不错").is_none());
    }
}
