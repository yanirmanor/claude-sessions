use chrono::{DateTime, Local, Utc};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, Mode};

const MUTED_BLUE_GRAY: Color = Color::Rgb(140, 140, 160);
const DIM_GRAY: Color = Color::Rgb(90, 90, 100);
const SOFT_WHITE: Color = Color::Rgb(200, 200, 210);
const HIGHLIGHT_BG: Color = Color::Rgb(50, 50, 80);
const MSG_COUNT_COLOR: Color = Color::Yellow;
const BRANCH_COLOR: Color = Color::Cyan;
const SEPARATOR_COLOR: Color = Color::Rgb(60, 60, 70);

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // search bar
        Constraint::Min(5),   // list
        Constraint::Length(1), // footer
    ])
    .split(area);

    // Header
    let session_count = app.filtered_indices.len();
    let total = app.sessions.len();
    let count_text = if session_count == total {
        format!("({})", total)
    } else {
        format!("({}/{})", session_count, total)
    };
    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " Claude Sessions",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" - {} ", app.project_path),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(count_text, Style::default().fg(Color::Yellow)),
    ]));
    frame.render_widget(header, chunks[0]);

    // Search bar
    let search_style = match app.mode {
        Mode::Search => Style::default().fg(Color::Yellow),
        Mode::Normal => Style::default().fg(Color::DarkGray),
    };
    let cursor_char = match app.mode {
        Mode::Search => "|",
        Mode::Normal => "",
    };
    let search = Paragraph::new(Line::from(vec![
        Span::styled(" Search: ", search_style),
        Span::styled(&app.search_query, Style::default().fg(Color::White)),
        Span::styled(cursor_char, Style::default().fg(Color::Yellow)),
    ]));
    frame.render_widget(search, chunks[1]);

    // Session list
    let items: Vec<ListItem> = app
        .filtered_indices
        .iter()
        .enumerate()
        .map(|(list_idx, &idx)| {
            let session = &app.sessions[idx];
            let date_str = format_timestamp(&session.timestamp);
            let branch = session.git_branch.as_deref().unwrap_or("?");
            let short_id = &session.id[..std::cmp::min(8, session.id.len())];
            let is_empty = session.message_count == 0
                && session.timestamp.is_none()
                && session.git_branch.is_none();

            // Line 1: metadata (date, branch, id, message count)
            let line1 = if is_empty {
                Line::from(vec![
                    Span::styled(
                        format!("  {}  ", date_str),
                        Style::default()
                            .fg(DIM_GRAY)
                            .add_modifier(Modifier::ITALIC),
                    ),
                    Span::styled(
                        format!("{}  ", branch),
                        Style::default()
                            .fg(DIM_GRAY)
                            .add_modifier(Modifier::ITALIC),
                    ),
                    Span::styled(
                        short_id.to_string(),
                        Style::default().fg(DIM_GRAY),
                    ),
                    Span::styled(
                        format!("  {} msgs", session.message_count),
                        Style::default().fg(DIM_GRAY),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::styled(
                        format!("  {}  ", date_str),
                        Style::default().fg(MUTED_BLUE_GRAY),
                    ),
                    Span::styled(
                        format!("{}  ", branch),
                        Style::default()
                            .fg(BRANCH_COLOR)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        short_id.to_string(),
                        Style::default().fg(DIM_GRAY),
                    ),
                    Span::styled(
                        " · ",
                        Style::default().fg(SEPARATOR_COLOR),
                    ),
                    Span::styled(
                        format!("{} msgs", session.message_count),
                        Style::default().fg(MSG_COUNT_COLOR),
                    ),
                ])
            };

            // Line 2+: user message (word-wrapped if expanded and selected)
            let indent = "    ";
            let is_selected = app.list_state.selected() == Some(list_idx);
            let is_expanded = is_selected && app.expanded;
            let msg_style = if session.first_user_message == "(no message)" || is_empty {
                Style::default()
                    .fg(DIM_GRAY)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(SOFT_WHITE)
            };

            let msg_lines: Vec<Line> = if is_expanded {
                let available_width = area.width as usize;
                let wrap_width = available_width.saturating_sub(indent.len() + 4); // account for indent + highlight symbol + borders
                if wrap_width > 0 {
                    word_wrap(&session.first_user_message, wrap_width)
                        .into_iter()
                        .map(|line_text| {
                            Line::from(vec![Span::styled(
                                format!("{}{}", indent, line_text),
                                msg_style,
                            )])
                        })
                        .collect()
                } else {
                    vec![Line::from(vec![Span::styled(
                        format!("{}{}", indent, session.first_user_message),
                        msg_style,
                    )])]
                }
            } else {
                vec![Line::from(vec![Span::styled(
                    format!("{}{}", indent, session.first_user_message),
                    msg_style,
                )])]
            };

            // Add a separator line between items (except before the first)
            let mut lines = Vec::new();
            if list_idx > 0 {
                lines.push(Line::from(Span::styled(
                    "  ─────",
                    Style::default().fg(SEPARATOR_COLOR),
                )));
            }
            lines.push(line1);
            lines.extend(msg_lines);

            ListItem::new(lines)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::TOP))
        .highlight_style(
            Style::default()
                .bg(HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, chunks[2], &mut app.list_state);

    // Footer
    let footer_text = match app.mode {
        Mode::Normal => {
            " ↑↓/jk Navigate  →← Expand/Collapse  Enter Resume  / Search  q Quit"
        }
        Mode::Search => {
            " ↑↓ Navigate  Enter Resume  Esc Clear search  Type to filter"
        }
    };
    let footer = Paragraph::new(Span::styled(
        footer_text,
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(footer, chunks[3]);
}

fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current_line = String::new();
    for word in text.split_whitespace() {
        if current_line.is_empty() {
            if word.len() > max_width {
                // Break long words
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                current_line = remaining.to_string();
            } else {
                current_line = word.to_string();
            }
        } else if current_line.len() + 1 + word.len() > max_width {
            lines.push(current_line);
            current_line = word.to_string();
        } else {
            current_line.push(' ');
            current_line.push_str(word);
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn format_timestamp(ts: &Option<String>) -> String {
    match ts {
        Some(ts_str) => {
            if let Ok(dt) = ts_str.parse::<DateTime<Utc>>() {
                let local_dt = dt.with_timezone(&Local);
                let today = Local::now().date_naive();
                let yesterday = today - chrono::Duration::days(1);
                let session_date = local_dt.date_naive();

                if session_date == today {
                    format!("Today {}", local_dt.format("%H:%M"))
                } else if session_date == yesterday {
                    format!("Yesterday {}", local_dt.format("%H:%M"))
                } else {
                    local_dt.format("%b %-d, %Y %H:%M").to_string()
                }
            } else {
                ts_str.clone()
            }
        }
        None => "Unknown date".to_string(),
    }
}
