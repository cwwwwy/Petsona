//! Lightweight pet memory.
//!
//! This is deliberately not a chat transcript. It stores a small set of stable
//! facts, recent interaction events and greeting state in one JSON file.

use std::collections::BTreeMap;
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PersonaMemory {
    pub facts: Vec<Fact>,
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
        ("我不喜欢", "不喜欢"),
        ("我讨厌", "不喜欢"),
        ("我不想要", "不想要"),
        ("我喜欢", "喜欢"),
        ("我偏好", "偏好"),
        ("我爱", "喜欢"),
        ("我想要", "想要"),
        ("请叫我", "称呼"),
        ("我的名字是", "称呼"),
        ("我叫", "称呼"),
        ("我习惯", "习惯"),
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
        ("i like ", "喜欢"),
        ("i prefer ", "偏好"),
        ("i love ", "喜欢"),
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
            serde_json::from_str(&text).unwrap_or_default()
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
        let existing = persona
            .facts
            .iter_mut()
            .find(|fact| fact.key.eq_ignore_ascii_case(key));
        let fact = match existing {
            Some(fact) => {
                fact.value = value.to_string();
                fact.confidence = confidence;
                fact.updated_at = now;
                fact.source = source.to_string();
                fact.clone()
            }
            None => {
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
            }
        };
        persona
            .facts
            .sort_by_key(|fact| std::cmp::Reverse(fact.updated_at));
        persona.facts.truncate(MAX_FACTS);
        self.save_locked(&memory)?;
        Ok(fact)
    }

    pub fn forget_fact(&self, persona_id: &str, fact_id: &str) -> Result<bool> {
        let mut memory = self.inner.lock();
        let Some(persona) = memory.personas.get_mut(persona_id) else {
            return Ok(false);
        };
        let before = persona.facts.len();
        persona.facts.retain(|fact| fact.id != fact_id);
        let removed = persona.facts.len() != before;
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
        let Some(fact) = persona.facts.iter_mut().find(|fact| fact.id == fact_id) else {
            return Ok(None);
        };
        fact.key = key.to_string();
        fact.value = value.to_string();
        fact.confidence = confidence.clamp(0.0, 1.0);
        fact.updated_at = now;
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
        let merged = old
            .iter()
            .map(|fact| format!("{}：{}", fact.key, fact.value))
            .collect::<Vec<_>>()
            .join("；");

        let profile = persona
            .facts
            .iter()
            .find(|fact| fact.key == "画像" && fact.source == FACT_SOURCE_COMPRESSED)
            .cloned();
        let merged_count = old.len();
        let now = now_ms();
        let profile = match profile {
            Some(mut profile) => {
                profile.value = format!("{}；{}", profile.value, merged);
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
        let removed = persona.facts.len();
        persona.facts.clear();
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
        let facts = memory.list_facts("default");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].value, "拿铁");
        memory.forget_fact("default", &facts[0].id).unwrap();
        assert!(memory.list_facts("default").is_empty());
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

        // Compressing again folds into the same profile instead of duplicating it.
        assert_eq!(memory.compress_facts("default", 2).unwrap(), 1);
        let facts = memory.list_facts("default");
        assert_eq!(facts.iter().filter(|fact| fact.key == "画像").count(), 1);
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
        assert!(extract_preference("今天感觉不错").is_none());
    }
}
