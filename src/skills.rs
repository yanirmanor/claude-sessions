use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillScope {
    Global,
    Project,
}

#[derive(Clone, Debug)]
pub struct SkillEntry {
    pub name: String,
    pub scope: SkillScope,
    pub path: PathBuf,
    pub source_root: PathBuf,
    pub has_skill_md: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillsFilter {
    All,
    Global,
    Project,
}

#[derive(Clone, Debug, Default)]
pub struct SkillsPolicy {
    disabled: HashSet<String>,
}

#[derive(Serialize, Deserialize)]
struct SkillsPolicyFile {
    disabled: Vec<String>,
}

pub fn discover_skills(project_path: &Path) -> Vec<SkillEntry> {
    let mut entries = Vec::new();

    for root in global_skill_roots() {
        discover_from_root(&root, SkillScope::Global, &mut entries);
    }

    for root in project_skill_roots(project_path) {
        discover_from_root(&root, SkillScope::Project, &mut entries);
    }

    let mut seen: HashSet<String> = HashSet::new();
    entries.retain(|entry| {
        let scope = match entry.scope {
            SkillScope::Global => "global",
            SkillScope::Project => "project",
        };
        let key = format!("{}:{}", scope, entry.name);
        if seen.contains(&key) {
            false
        } else {
            seen.insert(key);
            true
        }
    });

    entries.sort_by(|a, b| {
        let scope_a = match a.scope {
            SkillScope::Project => 0,
            SkillScope::Global => 1,
        };
        let scope_b = match b.scope {
            SkillScope::Project => 0,
            SkillScope::Global => 1,
        };
        scope_a
            .cmp(&scope_b)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    entries
}

pub fn skill_key(skill: &SkillEntry) -> String {
    let scope = match skill.scope {
        SkillScope::Global => "global",
        SkillScope::Project => "project",
    };
    format!("{}:{}", scope, skill.name)
}

impl SkillsPolicy {
    pub fn is_enabled(&self, skill: &SkillEntry) -> bool {
        !self.disabled.contains(&skill_key(skill))
    }

    pub fn set_enabled(&mut self, skill: &SkillEntry, enabled: bool) {
        let key = skill_key(skill);
        if enabled {
            self.disabled.remove(&key);
        } else {
            self.disabled.insert(key);
        }
    }
}

pub fn skills_policy_path(project_path: &Path) -> PathBuf {
    project_path.join(".opencode").join("skills-policy.json")
}

pub fn load_skills_policy(project_path: &Path) -> SkillsPolicy {
    let path = skills_policy_path(project_path);
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return SkillsPolicy::default(),
    };
    let parsed: SkillsPolicyFile = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return SkillsPolicy::default(),
    };
    SkillsPolicy {
        disabled: parsed.disabled.into_iter().collect(),
    }
}

pub fn save_skills_policy(project_path: &Path, policy: &SkillsPolicy) -> std::io::Result<()> {
    let path = skills_policy_path(project_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut disabled: Vec<String> = policy.disabled.iter().cloned().collect();
    disabled.sort();
    let payload = SkillsPolicyFile { disabled };
    let json =
        serde_json::to_string_pretty(&payload).map_err(|e| std::io::Error::other(e.to_string()))?;
    fs::write(path, json)
}

fn discover_from_root(root: &Path, scope: SkillScope, out: &mut Vec<SkillEntry>) {
    if !root.exists() || !root.is_dir() {
        return;
    }

    let read_dir = match fs::read_dir(root) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    for item in read_dir {
        let entry = match item {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        let has_skill_md = path.join("SKILL.md").exists();
        out.push(SkillEntry {
            name,
            scope: scope.clone(),
            path,
            source_root: root.to_path_buf(),
            has_skill_md,
        });
    }
}

fn global_skill_roots() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    vec![
        home.join(".agents").join("skills"),
        home.join(".config").join("opencode").join("skills"),
        home.join(".config").join("opencode").join("skill"),
    ]
}

fn project_skill_roots(project_path: &Path) -> Vec<PathBuf> {
    vec![
        project_path.join(".agents").join("skills"),
        project_path.join(".opencode").join("skills"),
        project_path.join(".opencode").join("skill"),
    ]
}
