//! Single DeepSeek API client.
//!
//! The rebuilt application only needs one short, non-streaming request for a
//! greeting. DeepSeek also supports Responses and Anthropic-shaped APIs, but
//! those transports are intentionally not part of this first version.

use std::time::Duration;

use serde_json::{json, Value};

use crate::config::{DeepSeekConfig, PROVIDER_CUSTOM, PROVIDER_DEEPSEEK};
use crate::error::{Error, Result};
use crate::memory::{now_ms, GreetingContext};
use crate::persona::Persona;
use crate::secrets::{KeyringStore, SecretStore};

pub struct DeepSeekClient {
    config: DeepSeekConfig,
    api_key: String,
    agent: ureq::Agent,
}

impl DeepSeekClient {
    /// Build a client using `DEEPSEEK_API_KEY` or the OS keychain entry
    /// `deepseek`.
    pub fn new(config: DeepSeekConfig) -> Result<Self> {
        let api_key = resolve_api_key(&config)?;
        Self::with_api_key(config, api_key)
    }

    pub fn with_api_key(config: DeepSeekConfig, api_key: String) -> Result<Self> {
        if api_key.trim().is_empty() {
            return Err(Error::ProviderNotConfigured(
                "DeepSeek API key is empty".to_string(),
            ));
        }
        let agent_config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(
                config.timeout_seconds.clamp(5, 120),
            )))
            .build();
        Ok(Self {
            config,
            api_key: api_key.trim().to_string(),
            agent: agent_config.new_agent(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn generate_greeting(
        &self,
        persona: &Persona,
        context: &GreetingContext,
        trigger: &str,
        now_text: &str,
        pet_name: Option<&str>,
        pet_state: &str,
        max_chars: usize,
    ) -> Result<String> {
        let (system, user) = build_prompt(persona, context, trigger, now_text, pet_name, pet_state);

        self.complete(&system, &user, max_chars.clamp(1, 200))
    }

    /// Generate a short conversational reply using the same persona and
    /// memory context as proactive greetings.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_reply(
        &self,
        persona: &Persona,
        context: &GreetingContext,
        history: &str,
        user_message: &str,
        now_text: &str,
        pet_name: Option<&str>,
        pet_state: &str,
    ) -> Result<String> {
        let system = format!(
            "{}\n\n你正在和用户进行桌面宠物对话。保持既定人格，直接回答用户，避免解释系统规则。\
             如果用户表达了稳定的喜好、习惯或厌恶，可以自然地在后续对话中参考；不要声称你记住了\
             用户没有明确表达的事情。回答使用用户的语言，通常不超过 120 个汉字。",
            persona.effective_system_prompt(pet_name, Some(pet_state))
        );
        let mut user =
            format!("当前时间：{now_text}\n宠物状态：{pet_state}\n用户消息：{user_message}\n");
        let memory = context.render(now_ms());
        if !memory.trim().is_empty() {
            user.push_str("\n记忆：\n");
            user.push_str(memory.trim());
            user.push('\n');
        }
        if !history.trim().is_empty() {
            user.push_str("\n最近对话：\n");
            user.push_str(history.trim());
            user.push('\n');
        }
        user.push_str("\n请直接回复用户，不要输出 Markdown 标题或动作描述。");
        self.complete(&system, &user, 400)
    }

    /// Request body for one completion. The DeepSeek-only `thinking` field is
    /// gated by the provider: strict OpenAI-compatible endpoints reject unknown
    /// fields, so a custom endpoint must never receive it (REQ-S16).
    pub fn request_body(&self, system: &str, user: &str) -> Value {
        let mut body = json!({
            "model": self.config.model.clone(),
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "stream": false,
            "max_tokens": self.config.max_tokens.clamp(16, 4000),
            "temperature": self.config.temperature.clamp(0.0, 2.0),
        });
        if self.config.thinking_disabled && self.config.provider != PROVIDER_CUSTOM {
            body["thinking"] = json!({ "type": "disabled" });
        }
        body
    }

    fn complete(&self, system: &str, user: &str, max_chars: usize) -> Result<String> {
        let body = self.request_body(system, user);

        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let response = self
            .agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|error| Error::provider(format!("DeepSeek request failed: {error}")))?;
        let value: Value = response
            .into_body()
            .read_json()
            .map_err(|error| Error::provider(format!("DeepSeek response was not JSON: {error}")))?;

        let text = value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            return Err(Error::provider(
                "DeepSeek returned an empty greeting".to_string(),
            ));
        }
        Ok(clean_greeting(text, max_chars))
    }
    /// `GET {base}/models` — the OpenAI-compatible model list. Custom endpoints
    /// that do not implement it surface a readable error and the UI keeps the
    /// manual model field (REQ-S16b).
    pub fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));
        let response = self
            .agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .call()
            .map_err(|error| Error::provider(format!("model list request failed: {error}")))?;
        let value: Value = response
            .into_body()
            .read_json()
            .map_err(|error| Error::provider(format!("model list was not JSON: {error}")))?;
        Ok(parse_model_ids(&value))
    }
}

/// Extract model ids from an OpenAI-compatible `/models` payload, sorted and
/// deduplicated. Pure so it can be unit-tested without a network.
pub fn parse_model_ids(value: &Value) -> Vec<String> {
    let mut ids: Vec<String> = value
        .get("data")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("id").and_then(Value::as_str))
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids.dedup();
    ids
}

/// Slot used before per-provider credentials existed.
pub const LEGACY_KEY_SLOT: &str = "deepseek";

/// Keychain slot for one provider, so switching providers never overwrites the
/// other one's credential (REQ-S16).
pub fn api_key_slot(provider: &str) -> String {
    let provider = if provider == PROVIDER_CUSTOM {
        PROVIDER_CUSTOM
    } else {
        PROVIDER_DEEPSEEK
    };
    format!("provider/{provider}")
}

/// Slots consulted for one provider, most specific first. DeepSeek keeps
/// working with a key saved before provider slots existed.
fn key_candidates(provider: &str) -> Vec<&'static str> {
    if provider == PROVIDER_CUSTOM {
        vec!["provider/custom"]
    } else {
        vec!["provider/deepseek", LEGACY_KEY_SLOT]
    }
}

/// Whether a usable credential exists for this config (env var or keychain).
/// Used by the settings page to show 已配置 / 未配置 without reading the secret.
pub fn api_key_present(config: &DeepSeekConfig) -> bool {
    let keyring = KeyringStore::new("com.petsona.desktop");
    api_key_present_with_store(config, &keyring)
}

/// Check credential presence with an injected store. Platform startup uses
/// [`api_key_present`] and the OS keychain; tests can use `MemorySecretStore`
/// to cover both configured and unconfigured paths without touching user data.
pub fn api_key_present_with_store(config: &DeepSeekConfig, store: &dyn SecretStore) -> bool {
    if std::env::var(&config.api_key_env)
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }

    key_candidates(&config.provider).iter().any(|slot| {
        store
            .get(slot)
            .map(|key| key.is_some_and(|key| !key.trim().is_empty()))
            .unwrap_or(false)
    })
}

pub fn resolve_api_key(config: &DeepSeekConfig) -> Result<String> {
    if let Ok(key) = std::env::var(&config.api_key_env) {
        if !key.trim().is_empty() {
            return Ok(key);
        }
    }

    let keyring = KeyringStore::new("com.petsona.desktop");
    for slot in key_candidates(&config.provider) {
        if let Some(key) = keyring.get(slot)? {
            if !key.trim().is_empty() {
                return Ok(key);
            }
        }
    }

    Err(Error::ProviderNotConfigured(format!(
        "set {} or save a key in the OS keychain",
        config.api_key_env
    )))
}

/// Save (or, with an empty key, delete) the credential of one provider.
///
/// Clearing must drop **every** slot the provider can read, including the
/// pre-provider `deepseek` entry: deleting only the new slot left the old key
/// in place, so "清除密钥" looked like a no-op (W19 regression).
pub fn save_api_key(provider: &str, key: &str) -> Result<()> {
    let keyring = KeyringStore::new("com.petsona.desktop");
    if key.trim().is_empty() {
        for slot in key_candidates(provider) {
            keyring.delete(slot)?;
        }
        return Ok(());
    }

    keyring.set(api_key_slot(provider).as_str(), key.trim())
}

fn build_prompt(
    persona: &Persona,
    context: &GreetingContext,
    trigger: &str,
    now_text: &str,
    pet_name: Option<&str>,
    pet_state: &str,
) -> (String, String) {
    let system = format!(
        "{}\n\n你只需要生成一句自然、简短、符合人格的问候。\
         不要输出解释、引号、Markdown 或括号中的动作描述。\
         不要重复上一次问候，不要编造没有出现在记忆里的信息。",
        persona.effective_system_prompt(pet_name, Some(pet_state))
    );

    let memory = context.render(now_ms());
    let mut user = format!("当前时间：{now_text}\n触发原因：{trigger}\n宠物状态：{pet_state}\n");
    if !memory.trim().is_empty() {
        user.push_str("\n记忆：\n");
        user.push_str(memory.trim());
        user.push('\n');
    }
    user.push_str("\n请生成一句中文问候，通常不超过 30 个汉字。只输出问候本身。");
    (system, user)
}

fn clean_greeting(text: &str, max_chars: usize) -> String {
    let mut out = text
        .trim()
        .trim_matches('"')
        .trim_matches('“')
        .trim_matches('”')
        .trim()
        .to_string();
    if let Some(first_line) = out.lines().find(|line| !line.trim().is_empty()) {
        out = first_line.trim().to_string();
    }
    if out.chars().count() > max_chars {
        out = out.chars().take(max_chars).collect::<String>();
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemorySecretStore;

    #[test]
    fn cleans_quotes_and_limits_length() {
        assert_eq!(clean_greeting("“你好呀”", 40), "你好呀");
        assert!(clean_greeting(&"啊".repeat(100), 10).ends_with('…'));
    }

    fn client(provider: &str) -> DeepSeekClient {
        let config = DeepSeekConfig {
            provider: provider.to_string(),
            ..DeepSeekConfig::default()
        };
        DeepSeekClient::with_api_key(config, "sk-test".to_string()).unwrap()
    }

    /// REQ-S16: strict OpenAI-compatible endpoints reject DeepSeek's `thinking`
    /// field, so it must only be sent for the built-in provider.
    #[test]
    fn thinking_field_is_deepseek_only() {
        assert!(client(PROVIDER_DEEPSEEK).request_body("s", "u")["thinking"].is_object());
        let custom = client(PROVIDER_CUSTOM).request_body("s", "u");
        assert!(
            custom.get("thinking").is_none(),
            "custom endpoints must not receive the thinking field"
        );
        assert_eq!(custom["model"], DeepSeekConfig::default().model);
    }

    #[test]
    fn parses_and_sorts_model_ids() {
        let payload = serde_json::json!({
            "object": "list",
            "data": [
                { "id": "deepseek-v4-flash" },
                { "id": " deepseek-v4 " },
                { "id": "deepseek-v4-flash" },
                { "id": "" },
                { "name": "no-id" }
            ]
        });
        assert_eq!(
            parse_model_ids(&payload),
            vec!["deepseek-v4".to_string(), "deepseek-v4-flash".to_string()]
        );
        // Unexpected shapes must not panic, they just yield nothing.
        assert!(parse_model_ids(&serde_json::json!({"data": "nope"})).is_empty());
        assert!(parse_model_ids(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn clearing_covers_every_readable_slot() {
        // "清除密钥" must also drop the legacy slot, otherwise resolve_api_key
        // still finds it and the UI keeps saying 已配置.
        assert_eq!(
            key_candidates(PROVIDER_DEEPSEEK),
            vec!["provider/deepseek", LEGACY_KEY_SLOT]
        );
        assert_eq!(key_candidates(PROVIDER_CUSTOM), vec!["provider/custom"]);
    }

    #[test]
    fn api_key_slots_are_per_provider() {
        assert_eq!(api_key_slot(PROVIDER_DEEPSEEK), "provider/deepseek");
        assert_eq!(api_key_slot(PROVIDER_CUSTOM), "provider/custom");
        // An unknown provider must never invent a third slot.
        assert_eq!(api_key_slot("openai"), "provider/deepseek");
    }

    #[test]
    fn credential_presence_supports_injected_empty_and_populated_stores() {
        let config = DeepSeekConfig {
            provider: PROVIDER_CUSTOM.to_string(),
            api_key_env: format!("PETSONA_NO_SUCH_KEY_{}", std::process::id()),
            ..DeepSeekConfig::default()
        };
        let empty = MemorySecretStore::new();
        assert!(!api_key_present_with_store(&config, &empty));

        let populated = MemorySecretStore::new();
        populated
            .set("provider/custom", "test-only-placeholder")
            .unwrap();
        assert!(api_key_present_with_store(&config, &populated));
    }
}
