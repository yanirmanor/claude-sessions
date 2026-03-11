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

#[derive(Deserialize, Debug)]
struct CodexJsonlEntry {
    #[serde(rename = "type")]
    entry_type: Option<String>,
    role: Option<String>,
    content: Option<serde_json::Value>,
    cwd: Option<String>,
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
                        first_user_message = truncate_str(&text, 120);
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

    for entry in WalkDir::new(&codex_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
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

        let entry: CodexJsonlEntry = match serde_json::from_str(&line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Check cwd for project filtering
        if !found_cwd {
            if let Some(ref cwd) = entry.cwd {
                found_cwd = true;
                if cwd.starts_with(project_path_str.as_ref())
                    || project_path_str.starts_with(cwd.as_str())
                {
                    matches_project = true;
                }
            }
        }

        // Count messages and extract first user message
        let role = entry.role.as_deref();
        if role == Some("user") || role == Some("assistant") {
            message_count += 1;
        }

        if first_user_message.is_empty() && role == Some("user") {
            if let Some(ref content) = entry.content {
                let text = match content {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Array(arr) => {
                        arr.iter().find_map(|item| {
                            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                                item.get("text").and_then(|t| t.as_str()).map(String::from)
                            } else {
                                None
                            }
                        })
                    }
                    _ => None,
                };
                if let Some(t) = text {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        first_user_message = truncate_str(trimmed, 120);
                    }
                }
            }
        }

        // Use first available timestamp-like field or derive from file metadata
        if timestamp.is_none() {
            if let Some(ref ts) = entry.entry_type {
                // Some Codex entries may have timestamp info; we'll fall back to file mod time
                let _ = ts;
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

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}
