use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use petsona_core::config::DeepSeekConfig;
use petsona_core::deepseek::DeepSeekClient;
use petsona_core::memory::now_ms;
use serde::{Deserialize, Serialize};

use crate::session::ConversationTurn;

const HISTORY_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct ConversationStore {
    directory: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationFile {
    version: u32,
    pet_id: String,
    turns: Vec<ConversationTurn>,
}

impl ConversationStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn load(&self, pet_id: &str) -> Result<Vec<ConversationTurn>> {
        let path = self.path(pet_id);
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read conversation history {}", path.display()))?;
        let file: ConversationFile = serde_json::from_str(&text)
            .with_context(|| format!("cannot parse conversation history {}", path.display()))?;
        anyhow::ensure!(
            file.version == HISTORY_VERSION && file.pet_id == pet_id,
            "conversation history identity or version mismatch"
        );
        Ok(file.turns)
    }

    pub fn save(&self, pet_id: &str, turns: &[ConversationTurn]) -> Result<()> {
        std::fs::create_dir_all(&self.directory).with_context(|| {
            format!(
                "cannot create conversation history directory {}",
                self.directory.display()
            )
        })?;
        let path = self.path(pet_id);
        // Refuse to replace a transcript we cannot read or whose identity is
        // mismatched. The user can explicitly clear the old file after
        // reviewing the visible load error.
        if path.exists() {
            self.load(pet_id)?;
        }
        let temporary = path.with_extension("json.tmp");
        let file = ConversationFile {
            version: HISTORY_VERSION,
            pet_id: pet_id.to_string(),
            turns: turns.to_vec(),
        };
        let text = serde_json::to_vec_pretty(&file)?;
        std::fs::write(&temporary, text).with_context(|| {
            format!("cannot write conversation history {}", temporary.display())
        })?;
        std::fs::rename(&temporary, &path)
            .with_context(|| format!("cannot replace conversation history {}", path.display()))
    }

    pub fn clear(&self, pet_id: &str) -> Result<()> {
        let path = self.path(pet_id);
        if path.exists() {
            std::fs::remove_file(&path).with_context(|| {
                format!("cannot remove conversation history {}", path.display())
            })?;
        }
        Ok(())
    }

    pub fn path(&self, pet_id: &str) -> PathBuf {
        // Pet ids are encoded instead of interpolated into a path.
        let encoded = pet_id
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        self.directory.join(format!("{encoded}.json"))
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }
}

pub fn stream_completion<F>(
    client: &DeepSeekClient,
    config: &DeepSeekConfig,
    system: &str,
    user: &str,
    cancelled: Arc<AtomicBool>,
    mut on_chunk: F,
) -> std::result::Result<(), String>
where
    F: FnMut(String) -> bool,
{
    let body = client.streaming_conversation_request_body(system, user);
    let timeout = Duration::from_secs(config.timeout_seconds.clamp(5, 120));
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .new_agent();
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let response = agent
        .post(&url)
        .header("Authorization", &format!("Bearer {}", client.api_key()))
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|error| format!("模型请求失败：{error}"))?;
    let status = response.status().as_u16();
    if status >= 400 {
        let body = response
            .into_body()
            .read_to_string()
            .unwrap_or_else(|_| String::new());
        let detail = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|payload| {
                payload
                    .pointer("/error/message")
                    .and_then(|message| message.as_str())
                    .map(str::to_string)
            })
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| body.trim().chars().take(300).collect());
        return Err(if detail.is_empty() {
            format!("模型请求失败（HTTP {status}）")
        } else {
            format!("模型请求失败（HTTP {status}）：{detail}")
        });
    }

    let mut reader = BufReader::new(response.into_body().into_reader());
    let mut pending = Vec::new();
    let mut generated = String::new();
    let mut done = false;
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err("已停止生成".to_string());
        }
        let count = reader
            .read_until(b'\n', &mut pending)
            .map_err(|error| format!("读取模型流失败：{error}"))?;
        if count == 0 {
            break;
        }
        if pending.len() > 1024 * 1024 {
            return Err("模型流单行超过大小上限".to_string());
        }
        // SSE is line framed, so a multibyte UTF-8 scalar can be split across
        // socket reads without being decoded until the complete event arrives.
        let line_end = pending.iter().position(|byte| *byte == b'\n');
        let Some(line_end) = line_end else { continue };
        let mut line = pending.drain(..=line_end).collect::<Vec<_>>();
        if line.last() == Some(&b'\n') {
            line.pop();
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let line =
            std::str::from_utf8(&line).map_err(|error| format!("模型流包含无效 UTF-8：{error}"))?;
        let data = match line.strip_prefix("data:") {
            Some(data) => data.trim(),
            None => continue,
        };
        if data == "[DONE]" {
            done = true;
            break;
        }
        let event: serde_json::Value = serde_json::from_str(data)
            .map_err(|error| format!("模型流事件不是有效 JSON：{error}"))?;
        if let Some(message) = event
            .pointer("/error/message")
            .and_then(|value| value.as_str())
        {
            return Err(format!("模型请求失败：{message}"));
        }
        if let Some(chunk) = event
            .pointer("/choices/0/delta/content")
            .and_then(|value| value.as_str())
        {
            if chunk.is_empty() {
                continue;
            }
            generated.push_str(chunk);
            if !on_chunk(chunk.to_string()) {
                return Err("生成任务已结束".to_string());
            }
        }
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("已停止生成".to_string());
    }
    if !done {
        return Err("模型流提前结束，没有收到完成标记".to_string());
    }
    if generated.trim().is_empty() {
        return Err("模型返回了空回复".to_string());
    }
    Ok(())
}

impl ConversationTurn {
    pub fn user(id: String, text: String) -> Self {
        Self {
            id,
            request_id: None,
            user: true,
            text,
            status: "complete".to_string(),
            created_at: now_ms(),
        }
    }

    pub fn assistant(request_id: String) -> Self {
        Self {
            id: request_id.clone(),
            request_id: Some(request_id),
            user: false,
            text: String::new(),
            status: "streaming".to_string(),
            created_at: now_ms(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petsona_core::config::{DeepSeekConfig, PROVIDER_CUSTOM};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    fn stub_sse(response: String) -> (String, JoinHandle<()>) {
        stub_http(200, response)
    }

    fn stub_http(status: u16, response: String) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0_u8; 1];
            while !request.ends_with(b"\r\n\r\n") {
                if stream.read_exact(&mut byte).is_err() {
                    return;
                }
                request.push(byte[0]);
                if request.len() > 16 * 1024 {
                    return;
                }
            }
            let headers = String::from_utf8_lossy(&request).to_ascii_lowercase();
            let body_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            let mut request_body = vec![0; body_length];
            if stream.read_exact(&mut request_body).is_err() {
                return;
            }
            let header = format!(
                "HTTP/1.1 {status} Test\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            );
            if stream.write_all(header.as_bytes()).is_err() {
                return;
            }
            // Deliberately split every byte; the SSE parser must wait for a
            // full newline-delimited event before decoding UTF-8 and JSON.
            for value in response.as_bytes() {
                if stream.write_all(std::slice::from_ref(value)).is_err() {
                    return;
                }
            }
        });
        (format!("http://127.0.0.1:{port}/v1"), worker)
    }

    #[test]
    fn history_is_isolated_by_pet_and_survives_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(temp.path().join("conversations"));
        let turns = vec![
            ConversationTurn::user("u1".into(), "我喜欢美式咖啡".into()),
            ConversationTurn {
                id: "a1".into(),
                request_id: Some("request-1".into()),
                user: false,
                text: "记住了。".into(),
                status: "complete".into(),
                created_at: now_ms(),
            },
        ];
        store.save("pet/a", &turns).unwrap();

        let reopened = ConversationStore::new(temp.path().join("conversations"));
        assert_eq!(reopened.load("pet/a").unwrap(), turns);
        assert!(reopened.load("pet/b").unwrap().is_empty());
        reopened.clear("pet/a").unwrap();
        assert!(reopened.load("pet/a").unwrap().is_empty());
    }

    #[test]
    fn invalid_history_is_reported_without_removing_the_original_file() {
        let temp = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(temp.path().to_path_buf());
        std::fs::create_dir_all(store.directory()).unwrap();
        let path = store.path("pet");
        std::fs::write(&path, b"invalid json").unwrap();

        assert!(store.load("pet").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid json");
    }

    #[test]
    fn save_refuses_to_overwrite_an_invalid_history_file() {
        let temp = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(temp.path().to_path_buf());
        std::fs::create_dir_all(store.directory()).unwrap();
        let path = store.path("pet");
        std::fs::write(&path, b"invalid json").unwrap();

        let error = store.save("pet", &[]).unwrap_err();

        assert!(error
            .to_string()
            .contains("cannot parse conversation history"));
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid json");
    }

    #[test]
    fn streaming_completion_delivers_split_utf8_and_requires_done() {
        let response = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let (base_url, server) = stub_sse(response.to_string());
        let config = DeepSeekConfig {
            provider: PROVIDER_CUSTOM.to_string(),
            base_url,
            api_key_env: "PETSONA_RUNTIME_TEST_API_KEY".to_string(),
            ..DeepSeekConfig::default()
        };
        let client = DeepSeekClient::with_api_key(config.clone(), "test-key".to_string()).unwrap();
        let body = client.streaming_conversation_request_body("system", "user");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], config.conversation_max_tokens);
        assert!(body.get("thinking").is_none());

        let chunks = std::sync::Mutex::new(Vec::new());
        let result = stream_completion(
            &client,
            &config,
            "system",
            "user",
            Arc::new(AtomicBool::new(false)),
            |chunk| {
                chunks.lock().unwrap().push(chunk);
                true
            },
        );
        server.join().unwrap();
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(*chunks.lock().unwrap(), vec!["你", "好"]);
    }

    #[test]
    fn streaming_completion_rejects_an_unterminated_response() {
        let response = "data: {\"choices\":[{\"delta\":{\"content\":\"片段\"}}]}\n\n";
        let (base_url, server) = stub_sse(response.to_string());
        let config = DeepSeekConfig {
            provider: PROVIDER_CUSTOM.to_string(),
            base_url,
            api_key_env: "PETSONA_RUNTIME_TEST_API_KEY".to_string(),
            ..DeepSeekConfig::default()
        };
        let client = DeepSeekClient::with_api_key(config.clone(), "test-key".to_string()).unwrap();
        let result = stream_completion(
            &client,
            &config,
            "system",
            "user",
            Arc::new(AtomicBool::new(false)),
            |_| true,
        );
        server.join().unwrap();
        assert!(result.unwrap_err().contains("完成标记"));
    }

    #[test]
    fn streaming_completion_surfaces_provider_http_error_details() {
        let (base_url, server) = stub_http(
            401,
            r#"{"error":{"message":"invalid test key"}}"#.to_string(),
        );
        let config = DeepSeekConfig {
            provider: PROVIDER_CUSTOM.to_string(),
            base_url,
            api_key_env: "PETSONA_RUNTIME_TEST_API_KEY".to_string(),
            ..DeepSeekConfig::default()
        };
        let client = DeepSeekClient::with_api_key(config.clone(), "test-key".to_string()).unwrap();
        let result = stream_completion(
            &client,
            &config,
            "system",
            "user",
            Arc::new(AtomicBool::new(false)),
            |_| true,
        );
        server.join().unwrap();
        let error = result.unwrap_err();
        assert!(error.contains("401"));
        assert!(error.contains("invalid test key"));
    }

    #[test]
    fn streaming_completion_stops_after_the_current_chunk_when_cancelled() {
        let response = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"第一段\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"第二段\"}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let (base_url, server) = stub_sse(response.to_string());
        let config = DeepSeekConfig {
            provider: PROVIDER_CUSTOM.to_string(),
            base_url,
            api_key_env: "PETSONA_RUNTIME_TEST_API_KEY".to_string(),
            ..DeepSeekConfig::default()
        };
        let client = DeepSeekClient::with_api_key(config.clone(), "test-key".to_string()).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let chunks = std::sync::Mutex::new(Vec::new());
        let cancel_on_chunk = Arc::clone(&cancelled);
        let result = stream_completion(&client, &config, "system", "user", cancelled, |chunk| {
            chunks.lock().unwrap().push(chunk);
            cancel_on_chunk.store(true, Ordering::Release);
            true
        });
        server.join().unwrap();
        assert!(result.unwrap_err().contains("已停止生成"));
        assert_eq!(chunks.lock().unwrap().as_slice(), ["第一段"]);
    }
}
