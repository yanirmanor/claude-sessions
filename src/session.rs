use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct Session {
    pub id: String,
    pub first_user_message: String,
    pub git_branch: Option<String>,
    pub timestamp: Option<String>,
    pub last_modified: SystemTime,
    pub message_count: usize,
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
    if !dir.exists() {
        anyhow::bail!("Sessions directory not found: {}", dir.display());
    }

    let mut sessions = Vec::new();

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
    })
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}
