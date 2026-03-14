use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

use crate::fuzzy::fuzzy_filter;
use crate::session::{CliTool, Session};
use crate::skills::{
    discover_skills, load_skills_policy, save_skills_policy, SkillEntry, SkillScope, SkillsFilter,
    SkillsPolicy,
};

pub enum Mode {
    Normal,
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Sessions,
    Stats,
    Skills,
}

pub enum Action {
    None,
    Quit,
    Resume(String, CliTool),
}

#[derive(Clone, Debug)]
pub enum ViewRow {
    Folder {
        path: String,
        label: String,
        depth: usize,
        count: usize,
        attachment_count: usize,
    },
    Session {
        session_idx: usize,
        depth: usize,
    },
}

pub struct App {
    pub sessions: Vec<Session>,
    pub filtered_indices: Vec<usize>,
    pub view_rows: Vec<ViewRow>,
    pub list_state: ListState,
    pub mode: Mode,
    pub screen: Screen,
    pub search_query: String,
    pub project_path: String,
    pub collapsed_folders: HashSet<String>,
    pub attachments_only: bool,
    pub skills: Vec<SkillEntry>,
    pub skills_policy: SkillsPolicy,
    pub skills_filter: SkillsFilter,
    pub skills_filtered_indices: Vec<usize>,
    pub skills_list_state: ListState,
    pub changed_files: Vec<String>,
}

impl App {
    pub fn new(sessions: Vec<Session>, project_path: String) -> Self {
        let filtered_indices: Vec<usize> = (0..sessions.len()).collect();
        let skills = discover_skills(Path::new(&project_path));
        let skills_policy = load_skills_policy(Path::new(&project_path));
        let mut app = Self {
            sessions,
            filtered_indices,
            view_rows: Vec::new(),
            list_state: ListState::default(),
            mode: Mode::Normal,
            screen: Screen::Sessions,
            search_query: String::new(),
            project_path,
            collapsed_folders: HashSet::new(),
            attachments_only: false,
            skills,
            skills_policy,
            skills_filter: SkillsFilter::All,
            skills_filtered_indices: Vec::new(),
            skills_list_state: ListState::default(),
            changed_files: Vec::new(),
        };
        app.refresh_changed_files();
        app.rebuild_view_rows();
        app.rebuild_skills_view();
        app
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }

        if key.code == KeyCode::Tab {
            self.cycle_screen();
            return Action::None;
        }

        match self.screen {
            Screen::Sessions => match self.mode {
                Mode::Normal => self.handle_sessions_normal_key(key),
                Mode::Search => self.handle_sessions_search_key(key),
            },
            Screen::Stats => self.handle_stats_key(key),
            Screen::Skills => self.handle_skills_key(key),
        }
    }

    fn cycle_screen(&mut self) {
        self.screen = match self.screen {
            Screen::Sessions => Screen::Stats,
            Screen::Stats => Screen::Skills,
            Screen::Skills => Screen::Sessions,
        };
        if self.screen != Screen::Sessions {
            self.mode = Mode::Normal;
        }
    }

    fn handle_sessions_normal_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Right => {
                if let Some(ViewRow::Folder { path, .. }) = self.selected_row() {
                    let folder = path.clone();
                    self.collapsed_folders.remove(&folder);
                    self.rebuild_view_rows();
                }
                Action::None
            }
            KeyCode::Left => {
                if let Some(ViewRow::Folder { path, .. }) = self.selected_row() {
                    let folder = path.clone();
                    self.collapsed_folders.insert(folder);
                    self.rebuild_view_rows();
                }
                Action::None
            }
            KeyCode::Char('[') => {
                self.collapse_all_folders();
                Action::None
            }
            KeyCode::Char(']') => {
                self.expand_all_folders();
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_prev();
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next();
                Action::None
            }
            KeyCode::Enter => self.resume_selected(),
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                Action::None
            }
            KeyCode::Char('a') => {
                self.attachments_only = !self.attachments_only;
                self.update_filter();
                Action::None
            }
            KeyCode::Char('R') => {
                self.refresh_changed_files();
                Action::None
            }
            KeyCode::Char(c) => {
                self.mode = Mode::Search;
                self.search_query.push(c);
                self.update_filter();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_sessions_search_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_query.clear();
                self.update_filter();
                Action::None
            }
            KeyCode::Enter => self.resume_selected(),
            KeyCode::Up | KeyCode::Char('\x10') => {
                self.select_prev();
                Action::None
            }
            KeyCode::Down | KeyCode::Char('\x0e') => {
                self.select_next();
                Action::None
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_filter();
                Action::None
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.update_filter();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_stats_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            _ => Action::None,
        }
    }

    fn handle_skills_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_prev_skill();
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next_skill();
                Action::None
            }
            KeyCode::Char('g') => {
                self.skills_filter = SkillsFilter::Global;
                self.rebuild_skills_view();
                Action::None
            }
            KeyCode::Char('p') => {
                self.skills_filter = SkillsFilter::Project;
                self.rebuild_skills_view();
                Action::None
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.skills_filter = SkillsFilter::All;
                self.rebuild_skills_view();
                Action::None
            }
            KeyCode::Char('r') => {
                self.reload_skills();
                Action::None
            }
            KeyCode::Char(' ') => {
                self.toggle_selected_skill();
                Action::None
            }
            KeyCode::Char('e') => {
                self.set_selected_skill_enabled(true);
                Action::None
            }
            KeyCode::Char('d') => {
                self.set_selected_skill_enabled(false);
                Action::None
            }
            KeyCode::Char('E') => {
                self.set_visible_skills_enabled(true);
                Action::None
            }
            KeyCode::Char('D') => {
                self.set_visible_skills_enabled(false);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn update_filter(&mut self) {
        let mut search_query = self.search_query.trim().to_string();
        let mut attachment_filter = self.attachments_only;

        if search_query == "att" || search_query.starts_with("att ") {
            attachment_filter = true;
            search_query = search_query
                .strip_prefix("att")
                .unwrap_or("")
                .trim()
                .to_string();
        }
        if search_query == "has:att" || search_query.starts_with("has:att ") {
            attachment_filter = true;
            search_query = search_query
                .strip_prefix("has:att")
                .unwrap_or("")
                .trim()
                .to_string();
        }

        self.filtered_indices = fuzzy_filter(&self.sessions, &search_query);
        if attachment_filter {
            self.filtered_indices
                .retain(|&idx| self.sessions[idx].attachment_count > 0);
        }
        self.rebuild_view_rows();
    }

    fn select_prev(&mut self) {
        if self.view_rows.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.view_rows.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn select_next(&mut self) {
        if self.view_rows.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.view_rows.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn select_prev_skill(&mut self) {
        if self.skills_filtered_indices.is_empty() {
            return;
        }
        let i = match self.skills_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.skills_filtered_indices.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.skills_list_state.select(Some(i));
    }

    fn select_next_skill(&mut self) {
        if self.skills_filtered_indices.is_empty() {
            return;
        }
        let i = match self.skills_list_state.selected() {
            Some(i) => {
                if i >= self.skills_filtered_indices.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.skills_list_state.select(Some(i));
    }

    fn reload_skills(&mut self) {
        self.skills = discover_skills(Path::new(&self.project_path));
        self.skills_policy = load_skills_policy(Path::new(&self.project_path));
        self.rebuild_skills_view();
    }

    fn refresh_changed_files(&mut self) {
        self.changed_files = git_changed_files(Path::new(&self.project_path));
    }

    fn toggle_selected_skill(&mut self) {
        let Some(skill_idx) = self.selected_skill_index() else {
            return;
        };
        let enabled = self.skills_policy.is_enabled(&self.skills[skill_idx]);
        self.skills_policy
            .set_enabled(&self.skills[skill_idx], !enabled);
        let _ = save_skills_policy(Path::new(&self.project_path), &self.skills_policy);
    }

    fn set_selected_skill_enabled(&mut self, enabled: bool) {
        let Some(skill_idx) = self.selected_skill_index() else {
            return;
        };
        self.skills_policy
            .set_enabled(&self.skills[skill_idx], enabled);
        let _ = save_skills_policy(Path::new(&self.project_path), &self.skills_policy);
    }

    fn set_visible_skills_enabled(&mut self, enabled: bool) {
        for &skill_idx in &self.skills_filtered_indices {
            self.skills_policy
                .set_enabled(&self.skills[skill_idx], enabled);
        }
        let _ = save_skills_policy(Path::new(&self.project_path), &self.skills_policy);
    }

    fn rebuild_skills_view(&mut self) {
        let selected_skill = self
            .selected_skill()
            .map(|skill| (skill.scope.clone(), skill.name.clone()));

        self.skills_filtered_indices = self
            .skills
            .iter()
            .enumerate()
            .filter_map(|(idx, skill)| {
                let matches = match self.skills_filter {
                    SkillsFilter::All => true,
                    SkillsFilter::Global => skill.scope == SkillScope::Global,
                    SkillsFilter::Project => skill.scope == SkillScope::Project,
                };
                if matches {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        if self.skills_filtered_indices.is_empty() {
            self.skills_list_state.select(None);
            return;
        }

        let mut target_idx = 0usize;
        if let Some((selected_scope, selected_name)) = selected_skill {
            if let Some(pos) = self.skills_filtered_indices.iter().position(|&idx| {
                self.skills[idx].scope == selected_scope && self.skills[idx].name == selected_name
            }) {
                target_idx = pos;
            }
        }
        self.skills_list_state.select(Some(target_idx));
    }

    fn resume_selected(&self) -> Action {
        if let Some(selected) = self.list_state.selected() {
            if let Some(ViewRow::Session { session_idx, .. }) = self.view_rows.get(selected) {
                let session = &self.sessions[*session_idx];
                return Action::Resume(session.id.clone(), session.tool.clone());
            }
        }
        Action::None
    }

    pub fn selected_row(&self) -> Option<&ViewRow> {
        let selected = self.list_state.selected()?;
        self.view_rows.get(selected)
    }

    pub fn selected_skill(&self) -> Option<&SkillEntry> {
        let skill_idx = self.selected_skill_index()?;
        self.skills.get(skill_idx)
    }

    pub fn selected_skill_index(&self) -> Option<usize> {
        let selected = self.skills_list_state.selected()?;
        self.skills_filtered_indices.get(selected).copied()
    }

    pub fn is_skill_enabled(&self, skill_idx: usize) -> bool {
        self.skills
            .get(skill_idx)
            .map(|skill| self.skills_policy.is_enabled(skill))
            .unwrap_or(false)
    }

    fn rebuild_view_rows(&mut self) {
        let selected_folder = match self.selected_row() {
            Some(ViewRow::Folder { path, .. }) => Some(path.clone()),
            _ => None,
        };
        let selected_session = match self.selected_row() {
            Some(ViewRow::Session { session_idx, .. }) => Some(*session_idx),
            _ => None,
        };

        self.view_rows.clear();

        let mut leaf_groups: HashMap<String, Vec<usize>> = HashMap::new();

        for &session_idx in &self.filtered_indices {
            let folder = self.sessions[session_idx]
                .relative_folder
                .as_deref()
                .unwrap_or("root")
                .to_string();
            leaf_groups.entry(folder).or_default().push(session_idx);
        }

        let mut all_folders: HashSet<String> = HashSet::new();
        for leaf in leaf_groups.keys() {
            all_folders.insert(leaf.clone());
            if leaf == "root" {
                continue;
            }
            let mut cur = leaf.as_str();
            while let Some((parent, _)) = cur.rsplit_once('/') {
                all_folders.insert(parent.to_string());
                cur = parent;
            }
        }

        let mut children: HashMap<Option<String>, Vec<String>> = HashMap::new();
        for folder in &all_folders {
            let parent = parent_folder(folder);
            children.entry(parent).or_default().push(folder.clone());
        }
        for vals in children.values_mut() {
            vals.sort();
        }

        if let Some(top) = children.get_mut(&None) {
            top.sort_by(|a, b| {
                if a == "root" && b != "root" {
                    std::cmp::Ordering::Less
                } else if b == "root" && a != "root" {
                    std::cmp::Ordering::Greater
                } else {
                    a.cmp(b)
                }
            });
        }

        fn subtree_counts(
            folder: &str,
            leaf_groups: &HashMap<String, Vec<usize>>,
            sessions: &[Session],
        ) -> (usize, usize) {
            let mut count = 0usize;
            let mut attachments = 0usize;
            for (leaf, list) in leaf_groups {
                let in_subtree = leaf == folder
                    || (folder != "root" && leaf.starts_with(&format!("{}/", folder)));
                if in_subtree {
                    count += list.len();
                    attachments += list
                        .iter()
                        .map(|&idx| sessions[idx].attachment_count)
                        .sum::<usize>();
                }
            }
            (count, attachments)
        }

        fn emit_folder(
            app: &mut App,
            folder: &str,
            depth: usize,
            children: &HashMap<Option<String>, Vec<String>>,
            leaf_groups: &HashMap<String, Vec<usize>>,
        ) {
            let label = folder
                .rsplit('/')
                .next()
                .map(|s| s.to_string())
                .unwrap_or_else(|| folder.to_string());
            let (count, attachment_count) = subtree_counts(folder, leaf_groups, &app.sessions);
            app.view_rows.push(ViewRow::Folder {
                path: folder.to_string(),
                label,
                depth,
                count,
                attachment_count,
            });

            if app.collapsed_folders.contains(folder) {
                return;
            }

            if let Some(session_indices) = leaf_groups.get(folder) {
                for &session_idx in session_indices {
                    app.view_rows.push(ViewRow::Session {
                        session_idx,
                        depth: depth + 1,
                    });
                }
            }

            if let Some(kids) = children.get(&Some(folder.to_string())) {
                for child in kids {
                    emit_folder(app, child, depth + 1, children, leaf_groups);
                }
            }
        }

        if let Some(top) = children.get(&None).cloned() {
            for folder in top {
                emit_folder(self, &folder, 0, &children, &leaf_groups);
            }
        }

        if self.view_rows.is_empty() {
            self.list_state.select(None);
            return;
        }

        let mut target_idx = 0usize;
        if let Some(session_idx) = selected_session {
            if let Some(i) = self.view_rows.iter().position(
                |row| matches!(row, ViewRow::Session { session_idx: s, .. } if *s == session_idx),
            ) {
                target_idx = i;
            }
        } else if let Some(folder) = selected_folder {
            if let Some(i) = self
                .view_rows
                .iter()
                .position(|row| matches!(row, ViewRow::Folder { path, .. } if path == &folder))
            {
                target_idx = i;
            }
        }
        self.list_state.select(Some(target_idx));
    }

    fn collapse_all_folders(&mut self) {
        self.collapsed_folders = self.all_folder_paths();
        self.rebuild_view_rows();
    }

    fn expand_all_folders(&mut self) {
        self.collapsed_folders.clear();
        self.rebuild_view_rows();
    }

    fn all_folder_paths(&self) -> HashSet<String> {
        let mut all_folders: HashSet<String> = HashSet::new();
        for &session_idx in &self.filtered_indices {
            let leaf = self.sessions[session_idx]
                .relative_folder
                .as_deref()
                .unwrap_or("root")
                .to_string();
            all_folders.insert(leaf.clone());
            if leaf == "root" {
                continue;
            }
            let mut cur = leaf.as_str();
            while let Some((parent, _)) = cur.rsplit_once('/') {
                all_folders.insert(parent.to_string());
                cur = parent;
            }
        }
        all_folders
    }
}

fn parent_folder(folder: &str) -> Option<String> {
    if folder == "root" {
        return None;
    }
    folder.rsplit_once('/').map(|(p, _)| p.to_string())
}

fn git_changed_files(project_path: &Path) -> Vec<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("status")
        .arg("--porcelain")
        .arg("--untracked-files=normal")
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            Some(line[3..].trim().to_string())
        })
        .collect()
}
