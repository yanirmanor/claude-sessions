use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::ListState;

use crate::fuzzy::fuzzy_filter;
use crate::session::{CliTool, Session};

pub enum Mode {
    Normal,
    Search,
}

pub enum Action {
    None,
    Quit,
    Resume(String, CliTool),
}

pub struct App {
    pub sessions: Vec<Session>,
    pub filtered_indices: Vec<usize>,
    pub list_state: ListState,
    pub mode: Mode,
    pub search_query: String,
    pub project_path: String,
    pub expanded: bool,
}

impl App {
    pub fn new(sessions: Vec<Session>, project_path: String) -> Self {
        let filtered_indices: Vec<usize> = (0..sessions.len()).collect();
        let mut list_state = ListState::default();
        if !filtered_indices.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            sessions,
            filtered_indices,
            list_state,
            mode: Mode::Normal,
            search_query: String::new(),
            project_path,
            expanded: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        // Ctrl+C always quits
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Action::Quit;
        }

        match self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::Search => self.handle_search_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Action::Quit,
            KeyCode::Right => {
                self.expanded = true;
                Action::None
            }
            KeyCode::Left => {
                self.expanded = false;
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.expanded = false;
                self.select_prev();
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.expanded = false;
                self.select_next();
                Action::None
            }
            KeyCode::Enter => self.resume_selected(),
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
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

    fn handle_search_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_query.clear();
                self.update_filter();
                Action::None
            }
            KeyCode::Enter => self.resume_selected(),
            KeyCode::Up | KeyCode::Char('\x10') => {
                // Up arrow or Ctrl+P
                self.select_prev();
                Action::None
            }
            KeyCode::Down | KeyCode::Char('\x0e') => {
                // Down arrow or Ctrl+N
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

    fn update_filter(&mut self) {
        self.filtered_indices = fuzzy_filter(&self.sessions, &self.search_query);
        if self.filtered_indices.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn select_prev(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_indices.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn select_next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= self.filtered_indices.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn resume_selected(&self) -> Action {
        if let Some(selected) = self.list_state.selected() {
            if let Some(&session_idx) = self.filtered_indices.get(selected) {
                let session = &self.sessions[session_idx];
                return Action::Resume(session.id.clone(), session.tool.clone());
            }
        }
        Action::None
    }
}
