use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

#[derive(Clone, Debug, PartialEq)]
pub enum CliTool {
    Claude,
    Codex,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub first_user_message: String,
    pub git_branch: Option<String>,
    pub timestamp: Option<String>,
    pub last_modified: SystemTime,
    pub message_count: usize,
    pub tool: CliTool,
}

#[derive(Deserialize, Debug)]
struct JsonlEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
    timestamp: Option<String>,
    message: Option<MessageObj>,
}

#[derive(Deserialize, Debug)]
struct MessageObj {
    role: Option<String>,
    content: Option<MessageContent>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Deserialize, Debug)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: Option<String>,
    text: Option<String>,
}

impl MessageContent {
    fn extract_text(&self) -> Option<String> {
        match self {
            MessageContent::Text(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            MessageContent::Blocks(blocks) => {
                for block in blocks {
                    if block.block_type.as_deref() == Some("text") {
                        if let Some(ref text) = block.text {
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
                None
            }
        }
    }
}

// --- Codex JSONL types ---

#[derive(Debug, Clone)]
struct CodexMessage {
    role: String,
    content: Option<serde_json::Value>,
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(|v| v.as_str())
}

fn extract_text_from_codex_content(content: &serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        serde_json::Value::Array(arr) => arr.iter().find_map(|item| {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            if let Some(nested) = item.get("content") {
                return extract_text_from_codex_content(nested);
            }
            None
        }),
        serde_json::Value::Object(obj) => {
            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            if let Some(message) = obj.get("message").and_then(|m| m.as_str()) {
                let trimmed = message.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            if let Some(nested) = obj.get("content") {
                return extract_text_from_codex_content(nested);
            }
            None
        }
        _ => None,
    }
}

fn codex_message_from_entry(entry: &serde_json::Value) -> Option<CodexMessage> {
    let entry_type = json_str(entry, "type");

    // New Codex format: {"type":"response_item","payload":{"type":"message","role":"user|assistant", ...}}
    if entry_type == Some("response_item") {
        let payload = entry.get("payload")?;
        if json_str(payload, "type") == Some("message") {
            let role = json_str(payload, "role")?.to_string();
            return Some(CodexMessage {
                role,
                content: payload.get("content").cloned(),
            });
        }

        // Alternate new format: {"type":"response_item","payload":{"type":"message","message":{"role":...,"content":...}}}
        if json_str(payload, "type") == Some("message") {
            if let Some(message_obj) = payload.get("message") {
                if let Some(role) = json_str(message_obj, "role") {
                    return Some(CodexMessage {
                        role: role.to_string(),
                        content: message_obj.get("content").cloned(),
                    });
                }
            }
        }
    }

    // Older/alternate format: {"role":"user|assistant", "content": ...}
    if let Some(role) = json_str(entry, "role") {
        return Some(CodexMessage {
            role: role.to_string(),
            content: entry.get("content").cloned(),
        });
    }

    None
}

fn codex_cwd_from_entry(entry: &serde_json::Value) -> Option<String> {
    // New format keeps cwd in session_meta payload
    if json_str(entry, "type") == Some("session_meta") {
        if let Some(payload) = entry.get("payload") {
            if let Some(cwd) = json_str(payload, "cwd") {
                return Some(cwd.to_string());
            }
        }
    }

    // Older format keeps cwd at top-level
    json_str(entry, "cwd").map(|s| s.to_string())
}

fn codex_user_message_from_event(entry: &serde_json::Value) -> Option<String> {
    if json_str(entry, "type") != Some("event_msg") {
        return None;
    }

    let payload = entry.get("payload")?;
    if json_str(payload, "type") != Some("user_message") {
        return None;
    }

    json_str(payload, "message")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn encode_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    s.replace('/', "-")
}

pub fn sessions_dir(project_path: &Path) -> Result<PathBuf> {
    let home = dirs::home_dir().context("Cannot determine home directory")?;
    let encoded = encode_path(project_path);
    let dir = home.join(".claude").join("projects").join(encoded);
    Ok(dir)
}

pub fn load_sessions(project_path: &Path) -> Result<Vec<Session>> {
    let dir = sessions_dir(project_path)?;

    let mut sessions = Vec::new();

    // Load Claude sessions
    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if path.is_dir() {
                continue;
            }

            match parse_session_file(&path) {
                Ok(session) => sessions.push(session),
                Err(e) => eprintln!("Warning: skipping {}: {}", path.display(), e),
            }
        }
    }

    // Load Codex sessions
    if let Ok(codex_sessions) = load_codex_sessions(project_path) {
        sessions.extend(codex_sessions);
    }

    if sessions.is_empty() {
        anyhow::bail!("No sessions found for {}", project_path.display());
    }

    sessions.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(sessions)
}

fn parse_session_file(path: &Path) -> Result<Session> {
    let metadata = fs::metadata(path)?;
    let last_modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let mut first_user_message = String::new();
    let mut git_branch: Option<String> = None;
    let mut timestamp: Option<String> = None;
    let mut message_count: usize = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }

        let entry: JsonlEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.entry_type.as_deref() == Some("file-history-snapshot") {
            continue;
        }

        if let Some(ref sid) = entry.session_id {
            if session_id == path.file_stem().and_then(|s| s.to_str()).unwrap_or("") {
                session_id = sid.clone();
            }
        }

        if git_branch.is_none() {
            if let Some(ref branch) = entry.git_branch {
                git_branch = Some(branch.clone());
            }
        }

        if let Some(ref msg) = entry.message {
            if msg.role.as_deref() == Some("user") || msg.role.as_deref() == Some("assistant") {
                message_count += 1;
            }

            if first_user_message.is_empty() && msg.role.as_deref() == Some("user") {
                if let Some(ref content) = msg.content {
                    if let Some(text) = content.extract_text() {
                        if let Some(cleaned) = sanitize_message(&text) {
                            first_user_message = cleaned;
                        }
                    }
                }
            }

            if timestamp.is_none() {
                if let Some(ref ts) = entry.timestamp {
                    timestamp = Some(ts.clone());
                }
            }
        }
    }

    if first_user_message.is_empty() {
        first_user_message = "(no message)".to_string();
    }

    Ok(Session {
        id: session_id,
        first_user_message,
        git_branch,
        timestamp,
        last_modified,
        message_count,
        tool: CliTool::Claude,
    })
}

// --- Codex session loading ---

fn load_codex_sessions(project_path: &Path) -> Result<Vec<Session>> {
    let home = dirs::home_dir().context("Cannot determine home directory")?;
    let codex_dir = home.join(".codex").join("sessions");

    if !codex_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();

    for entry in WalkDir::new(&codex_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        if path.is_dir() {
            continue;
        }

        match parse_codex_session_file(path, project_path) {
            Ok(Some(session)) => sessions.push(session),
            Ok(None) => {} // filtered out (different project)
            Err(_) => {}   // skip silently
        }
    }

    Ok(sessions)
}

fn parse_codex_session_file(path: &Path, project_path: &Path) -> Result<Option<Session>> {
    let metadata = fs::metadata(path)?;
    let last_modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut first_user_message = String::new();
    let mut timestamp: Option<String> = None;
    let mut message_count: usize = 0;
    let mut fallback_user_message_count: usize = 0;
    let mut found_cwd = false;
    let mut matches_project = false;

    let project_path_str = project_path.to_string_lossy();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }

        let entry: serde_json::Value = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Check cwd for project filtering
        if !found_cwd {
            if let Some(cwd) = codex_cwd_from_entry(&entry) {
                found_cwd = true;
                if cwd.starts_with(project_path_str.as_ref())
                    || project_path_str.starts_with(cwd.as_str())
                {
                    matches_project = true;
                }
            }
        }

        // Count messages and extract first user message
        if let Some(msg) = codex_message_from_entry(&entry) {
            if msg.role == "user" || msg.role == "assistant" {
                message_count += 1;
            }

            if first_user_message.is_empty() && msg.role == "user" {
                if let Some(content) = msg.content {
                    if let Some(text) = extract_text_from_codex_content(&content) {
                        if let Some(cleaned) = sanitize_message(&text) {
                            first_user_message = cleaned;
                        }
                    }
                }
            }
        }

        // Fallback for sessions that only emit event_msg user_message entries
        if let Some(user_message) = codex_user_message_from_event(&entry) {
            fallback_user_message_count += 1;
            if first_user_message.is_empty() {
                if let Some(cleaned) = sanitize_message(&user_message) {
                    first_user_message = cleaned;
                }
            }
        }

        // Use first available timestamp-like field or derive from file metadata
        if timestamp.is_none() {
            if let Some(ts) = json_str(&entry, "timestamp") {
                timestamp = Some(ts.to_string());
            }
        }
    }

    // Project filtering: if we found a cwd and it doesn't match, skip
    if found_cwd && !matches_project {
        return Ok(None);
    }

    if first_user_message.is_empty() {
        first_user_message = "(no message)".to_string();
    }

    if message_count == 0 && fallback_user_message_count > 0 {
        message_count = fallback_user_message_count;
    }

    // Derive session ID from filename (e.g., "rollout-abc123" from "rollout-abc123.jsonl")
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Use file modification time for timestamp display
    if timestamp.is_none() {
        if let Ok(duration) = last_modified.duration_since(SystemTime::UNIX_EPOCH) {
            let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(
                duration.as_secs() as i64,
                duration.subsec_nanos(),
            );
            if let Some(dt) = dt {
                timestamp = Some(dt.to_rfc3339());
            }
        }
    }

    Ok(Some(Session {
        id: session_id,
        first_user_message,
        git_branch: None,
        timestamp,
        last_modified,
        message_count,
        tool: CliTool::Codex,
    }))
}

fn sanitize_message(s: &str) -> Option<String> {
    let no_control: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    let normalized = no_control.split_whitespace().collect::<Vec<_>>().join(" ");

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_response_item_message() {
        let entry = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "hello from codex"}
                ]
            }
        });

        let msg = codex_message_from_entry(&entry).expect("expected message");
        assert_eq!(msg.role, "user");
        assert_eq!(
            msg.content
                .as_ref()
                .and_then(extract_text_from_codex_content)
                .as_deref(),
            Some("hello from codex")
        );
    }

    #[test]
    fn parses_codex_event_user_message_fallback() {
        let entry = serde_json::json!({
            "type": "event_msg",
            "payload": {
                "type": "user_message",
                "message": "Session name: improve codex support"
            }
        });

        assert_eq!(
            codex_user_message_from_event(&entry).as_deref(),
            Some("Session name: improve codex support")
        );
    }

    #[test]
    fn sanitizes_control_sequences_and_whitespace() {
        let raw = "  hello\x1b[2J\n\tworld  ";
        assert_eq!(sanitize_message(raw).as_deref(), Some("hello [2J world"));
    }
}
