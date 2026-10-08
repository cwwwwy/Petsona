use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

const MAX_SOURCE_MESSAGES: usize = 20_000;
const MAX_MESSAGE_CHARS: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonaChatMessage {
    pub id: String,
    pub speaker: String,
    pub text: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ParsedPersonaSource {
    pub id: String,
    pub label: String,
    pub format: String,
    pub messages: Vec<PersonaChatMessage>,
}

pub fn parse_json_chat(text: &str) -> Result<Vec<PersonaChatMessage>> {
    let parsed: Value = serde_json::from_str(text)
        .map_err(|error| Error::config(format!("聊天记录 JSON 无效：{error}")))?;
    let messages = parsed
        .as_array()
        .ok_or_else(|| Error::config("聊天记录 JSON 顶层必须是消息数组"))?;
    if messages.is_empty() || messages.len() > MAX_SOURCE_MESSAGES {
        return Err(Error::config("聊天记录消息数量必须在 1 到 20000 之间"));
    }
    let mut result = Vec::with_capacity(messages.len());
    for (index, value) in messages.iter().enumerate() {
        let speaker = value
            .get("speaker")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let body = value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let timestamp = value.get("timestamp").and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            _ => None,
        });
        result.push(message(index, speaker, body, timestamp)?);
    }
    Ok(result)
}

pub fn parse_text_chat(text: &str) -> Result<Vec<PersonaChatMessage>> {
    let mut messages: Vec<(String, String, Option<String>)> = Vec::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((speaker, body, timestamp)) = parse_speaker_line(trimmed) {
            messages.push((speaker.to_string(), body.to_string(), timestamp));
        } else if let Some((_, body, _)) = messages.last_mut() {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(trimmed);
        } else {
            return Err(Error::config("文本首条消息必须以“说话人: 消息内容”开头"));
        }
        if messages.len() > MAX_SOURCE_MESSAGES {
            return Err(Error::config("聊天记录超过 20000 条消息"));
        }
    }
    if messages.is_empty() {
        return Err(Error::config("聊天记录没有可识别的消息"));
    }
    messages
        .into_iter()
        .enumerate()
        .map(|(index, (speaker, text, timestamp))| message(index, &speaker, &text, timestamp))
        .collect()
}

fn parse_speaker_line(line: &str) -> Option<(&str, &str, Option<String>)> {
    let (line, timestamp) = if let Some((timestamp, rest)) = line
        .strip_prefix('[')
        .and_then(|value| value.split_once(']'))
    {
        let timestamp = timestamp.trim();
        let timestamp = (!timestamp.is_empty()).then(|| timestamp.to_string());
        (rest.trim_start(), timestamp)
    } else {
        (line, None)
    };
    let (speaker, text) = line.split_once(':')?;
    let speaker = speaker.trim();
    if speaker.is_empty() || speaker.chars().count() > 120 {
        return None;
    }
    Some((speaker, text.trim_start(), timestamp))
}

fn message(
    index: usize,
    speaker: &str,
    text: &str,
    timestamp: Option<String>,
) -> Result<PersonaChatMessage> {
    let speaker = speaker.trim();
    let text = text.trim();
    if speaker.is_empty() || speaker.chars().count() > 120 {
        return Err(Error::config(format!(
            "第 {} 条消息的说话人无效",
            index + 1
        )));
    }
    if text.is_empty() {
        return Err(Error::config(format!("第 {} 条消息内容为空", index + 1)));
    }
    if text.chars().count() > MAX_MESSAGE_CHARS {
        return Err(Error::config(format!(
            "第 {} 条消息超过长度上限",
            index + 1
        )));
    }
    Ok(PersonaChatMessage {
        id: format!("m-{}", index + 1),
        speaker: speaker.to_string(),
        text: text.to_string(),
        timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_message_array_and_optional_timestamps() {
        let records = parse_json_chat(
            r#"[{"speaker":"甲","text":"早上去散步","timestamp":"09:00"},{"speaker":"乙","text":"晚上喝茶","timestamp":17300}]"#,
        )
        .unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].speaker, "甲");
        assert_eq!(records[1].timestamp.as_deref(), Some("17300"));
    }

    #[test]
    fn parses_plain_text_speakers_and_continuation_lines() {
        let records = parse_text_chat("甲: 第一行\n继续\n乙: 回答\n").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].text, "第一行\n继续");
        assert_eq!(records[1].speaker, "乙");
        let timed = parse_text_chat("[09:30]甲: 早上散步").unwrap();
        assert_eq!(timed[0].timestamp.as_deref(), Some("09:30"));
    }

    #[test]
    fn invalid_or_oversized_sources_are_rejected() {
        assert!(parse_json_chat(r#"{"messages":[]}"#).is_err());
        assert!(parse_text_chat("没有说话人标题的消息").is_err());
        assert!(parse_json_chat("not json").is_err());
    }
}
