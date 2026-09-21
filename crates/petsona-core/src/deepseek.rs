//! Single DeepSeek API client.
//!
//! The rebuilt application only needs one short, non-streaming request for a
//! greeting. DeepSeek also supports Responses and Anthropic-shaped APIs, but
//! those transports are intentionally not part of this first version.

use std::time::Duration;

use serde_json::{json, Value};

use crate::config::DeepSeekConfig;
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

    fn complete(&self, system: &str, user: &str, max_chars: usize) -> Result<String> {
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
        if self.config.thinking_disabled {
            body["thinking"] = json!({ "type": "disabled" });
        }

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
}

pub fn resolve_api_key(config: &DeepSeekConfig) -> Result<String> {
    if let Ok(key) = std::env::var(&config.api_key_env) {
        if !key.trim().is_empty() {
            return Ok(key);
        }
    }

    let keyring = KeyringStore::new("com.petsona.desktop");
    match keyring.get("deepseek")? {
        Some(key) if !key.trim().is_empty() => Ok(key),
        _ => Err(Error::ProviderNotConfigured(format!(
            "set {} or save a key in the OS keychain",
            config.api_key_env
        ))),
    }
}

pub fn save_api_key(key: &str) -> Result<()> {
    KeyringStore::new("com.petsona.desktop").set("deepseek", key.trim())
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

    #[test]
    fn cleans_quotes_and_limits_length() {
        assert_eq!(clean_greeting("“你好呀”", 40), "你好呀");
        assert!(clean_greeting(&"啊".repeat(100), 10).ends_with('…'));
    }
}
