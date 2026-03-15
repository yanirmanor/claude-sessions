use chrono::{DateTime, Local, Utc};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use std::collections::HashMap;

use crate::app::{App, Mode, Screen, ViewRow};
use crate::session::CliTool;
use crate::skills::{SkillScope, SkillsFilter};

const MUTED_BLUE_GRAY: Color = Color::Rgb(140, 140, 160);
const DIM_GRAY: Color = Color::Rgb(90, 90, 100);
const SOFT_WHITE: Color = Color::Rgb(200, 200, 210);
const HIGHLIGHT_BG: Color = Color::Rgb(50, 50, 80);
const MSG_COUNT_COLOR: Color = Color::Yellow;
const BRANCH_COLOR: Color = Color::Cyan;
const FOLDER_COLOR: Color = Color::LightCyan;
const ATTACHMENT_COLOR: Color = Color::Magenta;
const SEPARATOR_COLOR: Color = Color::Rgb(60, 60, 70);

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .split(area);

    render_header(frame, app, chunks[0]);
    render_top_line(frame, app, chunks[1]);

    match app.screen {
        Screen::Sessions => render_sessions_screen(frame, app, chunks[2]),
        Screen::Stats => render_stats_screen(frame, app, chunks[2]),
        Screen::Skills => render_skills_screen(frame, app, chunks[2]),
    }

    render_footer(frame, app, chunks[3]);
}

fn render_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let session_count = app.filtered_indices.len();
    let total = app.sessions.len();
    let count_text = if session_count == total {
        format!("({})", total)
    } else {
        format!("({}/{})", session_count, total)
    };

    let screen_tag = match app.screen {
        Screen::Sessions => "[sessions]",
        Screen::Stats => "[stats]",
        Screen::Skills => "[skills]",
    };

    let mut spans = vec![
        Span::styled(
            " AI Sessions",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" - {} ", app.project_path),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(count_text, Style::default().fg(Color::Yellow)),
        Span::styled(format!(" {}", screen_tag), Style::default().fg(Color::Cyan)),
    ];

    if app.screen == Screen::Sessions && app.attachments_only {
        spans.push(Span::styled(
            " [att]",
            Style::default().fg(ATTACHMENT_COLOR),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_top_line(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    match app.screen {
        Screen::Sessions => {
            if app.view_rows.is_empty() {
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(" No sessions visible. ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            "Try Esc to clear search or press 'a' to disable attachments filter.",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ])),
                    area,
                );
                return;
            }

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
                Span::styled(
                    format!("  rows:{}", app.view_rows.len()),
                    Style::default().fg(Color::Gray),
                ),
            ]));
            frame.render_widget(search, area);
        }
        Screen::Stats => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    " Stats dashboard",
                    Style::default().fg(Color::DarkGray),
                )),
                area,
            );
        }
        Screen::Skills => {
            let filter_label = match app.skills_filter {
                SkillsFilter::All => "all",
                SkillsFilter::Global => "global",
                SkillsFilter::Project => "project",
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(" Skills filter: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(filter_label, Style::default().fg(Color::Yellow)),
                    Span::styled(
                        "  (space toggle, e/d one, E/D all, g/p/a filter, r refresh)",
                        Style::default().fg(Color::DarkGray),
                    ),
                ])),
                area,
            );
        }
    }
}

fn render_sessions_screen(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    if app.view_rows.is_empty() {
        let mut lines = vec![Line::from(vec![Span::styled(
            " No sessions to display",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )])];

        if app.attachments_only {
            lines.push(Line::from(
                " Attachments-only filter is enabled (press 'a' to disable).",
            ));
        }
        if !app.search_query.trim().is_empty() {
            lines.push(Line::from(format!(
                " Search query '{}' matched nothing (Esc to clear).",
                app.search_query
            )));
        }

        if !app.attachments_only && app.search_query.trim().is_empty() {
            lines.push(Line::from(
                " No sessions were found for this project path. Use Tab for Stats/Skills.",
            ));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            " Changed files in project",
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        if app.changed_files.is_empty() {
            lines.push(Line::from(" none"));
        } else {
            for file in app.changed_files.iter().take(10) {
                lines.push(Line::from(format!(" - {}", truncate_for_preview(file, 90))));
            }
            if app.changed_files.len() > 10 {
                lines.push(Line::from(format!(
                    " +{} more",
                    app.changed_files.len() - 10
                )));
            }
        }
        lines.push(Line::from(" Press Shift+R to refresh changed files."));

        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::TOP).title(" Sessions ")),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .view_rows
        .iter()
        .enumerate()
        .map(|(list_idx, row)| match row {
            ViewRow::Folder {
                path,
                label,
                depth,
                count,
                attachment_count,
            } => {
                let indent = "  ".repeat(*depth);
                let marker = if app.collapsed_folders.contains(path) {
                    "+"
                } else {
                    "-"
                };
                let mut spans = vec![
                    Span::styled(
                        format!(" {}[{}] {}", indent, marker, label),
                        Style::default()
                            .fg(FOLDER_COLOR)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {} sessions", count),
                        Style::default().fg(MSG_COUNT_COLOR),
                    ),
                ];
                if *attachment_count > 0 {
                    spans.push(Span::styled(
                        format!("  +{} att", attachment_count),
                        Style::default().fg(ATTACHMENT_COLOR),
                    ));
                }
                let folder_line = Line::from(spans);
                let separator = if list_idx > 0 {
                    Line::from(Span::styled(
                        format!(" {}───", indent),
                        Style::default().fg(SEPARATOR_COLOR),
                    ))
                } else {
                    Line::from("")
                };
                ListItem::new(vec![folder_line, separator])
            }
            ViewRow::Session { session_idx, depth } => {
                let session = &app.sessions[*session_idx];
                let date_str = format_timestamp(&session.timestamp);
                let branch = session.git_branch.as_deref().unwrap_or("?");
                let short_id = &session.id[..std::cmp::min(8, session.id.len())];
                let is_empty = session.message_count == 0
                    && session.timestamp.is_none()
                    && session.git_branch.is_none();

                let (tool_label, tool_color) = match session.tool {
                    CliTool::Claude => ("[Claude]", Color::Magenta),
                    CliTool::Codex => ("[Codex]", Color::Green),
                };

                let indent_prefix = "  ".repeat(*depth);

                let line1 = if is_empty {
                    let mut spans = vec![
                        Span::styled(
                            format!(" {}{} ", indent_prefix, tool_label),
                            Style::default().fg(tool_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{}  ", date_str),
                            Style::default().fg(DIM_GRAY).add_modifier(Modifier::ITALIC),
                        ),
                        Span::styled(
                            format!("{}  ", branch),
                            Style::default().fg(DIM_GRAY).add_modifier(Modifier::ITALIC),
                        ),
                        Span::styled(short_id.to_string(), Style::default().fg(DIM_GRAY)),
                        Span::styled(
                            format!("  {} msgs", session.message_count),
                            Style::default().fg(DIM_GRAY),
                        ),
                    ];
                    if session.attachment_count > 0 {
                        spans.push(Span::styled(
                            format!("  +{} att", session.attachment_count),
                            Style::default().fg(DIM_GRAY),
                        ));
                    }
                    Line::from(spans)
                } else {
                    let mut spans = vec![
                        Span::styled(
                            format!(" {}{} ", indent_prefix, tool_label),
                            Style::default().fg(tool_color).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!("{}  ", date_str),
                            Style::default().fg(MUTED_BLUE_GRAY),
                        ),
                        Span::styled(
                            format!("{}  ", branch),
                            Style::default()
                                .fg(BRANCH_COLOR)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(short_id.to_string(), Style::default().fg(DIM_GRAY)),
                        Span::styled(" · ", Style::default().fg(SEPARATOR_COLOR)),
                        Span::styled(
                            format!("{} msgs", session.message_count),
                            Style::default().fg(MSG_COUNT_COLOR),
                        ),
                    ];
                    if session.attachment_count > 0 {
                        spans.push(Span::styled(" · ", Style::default().fg(SEPARATOR_COLOR)));
                        spans.push(Span::styled(
                            format!("+{} att", session.attachment_count),
                            Style::default().fg(ATTACHMENT_COLOR),
                        ));
                    }
                    Line::from(spans)
                };

                let indent = format!("{}    ", indent_prefix);
                let msg_style = if session.first_user_message == "(no message)" || is_empty {
                    Style::default().fg(DIM_GRAY).add_modifier(Modifier::ITALIC)
                } else {
                    Style::default().fg(SOFT_WHITE)
                };

                let available_width = area.width as usize;
                let preview_width = available_width.saturating_sub(indent.len() + 4);
                let preview_text = if preview_width > 0 {
                    truncate_for_preview(&session.first_user_message, preview_width)
                } else {
                    session.first_user_message.clone()
                };
                let msg_lines: Vec<Line> = vec![Line::from(vec![Span::styled(
                    format!("{}{}", indent, preview_text),
                    msg_style,
                )])];

                let mut lines = Vec::new();
                lines.push(line1);
                lines.extend(msg_lines);
                ListItem::new(lines)
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::TOP))
        .highlight_style(
            Style::default()
                .bg(HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_stats_screen(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let stats = compute_dashboard_stats(app);
    let mut stats_lines = vec![
        Line::from(vec![
            Span::styled(" Today", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(
                format!("{} sessions", stats.today_sessions),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(format!(" Messages {}", stats.today_messages)),
        Line::from(format!(
            " Tokens in {} out {}",
            format_compact_u64(stats.today_input_tokens),
            format_compact_u64(stats.today_output_tokens)
        )),
        Line::from(format!(
            " Tools Claude {}  Codex {}",
            stats.today_claude, stats.today_codex
        )),
        Line::from(format!(" Cost ${:.2}", stats.today_cost_usd)),
        Line::from(format!(" Attachments {}", stats.today_attachments)),
        Line::from(""),
        Line::from(vec![Span::styled(
            " All Time",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!(
            " Sessions {}  Messages {}",
            stats.total_sessions, stats.total_messages
        )),
        Line::from(format!(
            " Tokens in {} out {}",
            format_compact_u64(stats.total_input_tokens),
            format_compact_u64(stats.total_output_tokens)
        )),
        Line::from(format!(" Cost ${:.2}", stats.total_cost_usd)),
        Line::from(format!(" Attachments {}", stats.total_attachments)),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Tool Mix (Today)",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(format!(
            " Claude {} {}",
            stats.today_claude,
            mini_bar(stats.today_claude, stats.max_tool_count, 20)
        )),
        Line::from(format!(
            " Codex  {} {}",
            stats.today_codex,
            mini_bar(stats.today_codex, stats.max_tool_count, 20)
        )),
        Line::from(""),
        Line::from(vec![Span::styled(
            " Top Folders (Today)",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
    ];

    if stats.top_folders.is_empty() {
        stats_lines.push(Line::from(" none"));
    } else {
        for (folder, count) in stats.top_folders {
            stats_lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    truncate_for_preview(&folder, 32),
                    Style::default().fg(FOLDER_COLOR),
                ),
                Span::raw(" "),
                Span::styled(format!("{}", count), Style::default().fg(Color::White)),
                Span::raw(" "),
                Span::styled(
                    mini_bar(count, stats.max_folder_count, 16),
                    Style::default().fg(Color::Cyan),
                ),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(stats_lines).block(Block::default().borders(Borders::TOP).title(" Stats ")),
        area,
    );
}

fn render_skills_screen(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let chunks = if area.width >= 120 {
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(area)
    } else {
        Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).split(area)
    };

    let items: Vec<ListItem> = app
        .skills_filtered_indices
        .iter()
        .map(|&skill_idx| {
            let skill = &app.skills[skill_idx];
            let enabled = app.is_skill_enabled(skill_idx);
            let scope_badge = match skill.scope {
                SkillScope::Global => "[G]",
                SkillScope::Project => "[P]",
            };
            let scope_color = match skill.scope {
                SkillScope::Global => Color::Green,
                SkillScope::Project => Color::Cyan,
            };
            let status = if skill.has_skill_md {
                "ok"
            } else {
                "missing SKILL.md"
            };
            let enabled_text = if enabled { "ON" } else { "OFF" };
            let enabled_color = if enabled { Color::Green } else { Color::Red };
            ListItem::new(vec![Line::from(vec![
                Span::styled(
                    format!(" {} ", scope_badge),
                    Style::default()
                        .fg(scope_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    skill.name.clone(),
                    Style::default().fg(SOFT_WHITE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", enabled_text),
                    Style::default()
                        .fg(enabled_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {}", status), Style::default().fg(DIM_GRAY)),
            ])])
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::TOP).title(" Skills "))
        .highlight_style(
            Style::default()
                .bg(HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    frame.render_stateful_widget(list, chunks[0], &mut app.skills_list_state);

    let details_lines = if let Some(skill) = app.selected_skill() {
        let enabled = app
            .selected_skill_index()
            .map(|idx| app.is_skill_enabled(idx))
            .unwrap_or(true);
        let scope = match skill.scope {
            SkillScope::Global => "Global",
            SkillScope::Project => "Project",
        };
        vec![
            Line::from(vec![
                Span::styled(" Name: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&skill.name, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled(" Scope: ", Style::default().fg(Color::DarkGray)),
                Span::styled(scope, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::styled(" Path: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    skill.path.to_string_lossy().to_string(),
                    Style::default().fg(SOFT_WHITE),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Root: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    skill.source_root.to_string_lossy().to_string(),
                    Style::default().fg(SOFT_WHITE),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Status: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if skill.has_skill_md {
                        "Ready"
                    } else {
                        "Missing SKILL.md"
                    },
                    Style::default().fg(if skill.has_skill_md {
                        Color::Green
                    } else {
                        Color::Red
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Use in project: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if enabled { "Enabled" } else { "Disabled" },
                    Style::default().fg(if enabled { Color::Green } else { Color::Red }),
                ),
            ]),
        ]
    } else {
        vec![Line::from(" No skills found for this filter")]
    };

    frame.render_widget(
        Paragraph::new(details_lines)
            .block(Block::default().borders(Borders::TOP).title(" Details ")),
        chunks[1],
    );
}

fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let footer_text = match app.screen {
        Screen::Sessions => match app.mode {
            Mode::Normal => {
                if app.view_rows.is_empty() {
                    " No rows  Esc clear search  a toggle attachments  Shift+R refresh files  Tab next screen  q Quit"
                } else {
                    " Up/Down or j/k Navigate  Left/Right Fold folder  [ Collapse all  ] Expand all  a Attachments  / Search  Enter Resume  Shift+R Refresh files  Tab Next screen  q Quit"
                }
            }
            Mode::Search => {
                " Up/Down Navigate  Enter Resume  Esc Clear search  Type to filter (att, has:att)  Tab Next screen"
            }
        },
        Screen::Stats => " Tab Next screen  q Quit",
        Screen::Skills => {
            " Up/Down Navigate  Space Toggle  e/d One  E/D All  g/p/a Filter  r Refresh  Tab Next screen  q Quit"
        }
    };
    frame.render_widget(
        Paragraph::new(Span::styled(
            footer_text,
            Style::default().fg(Color::DarkGray),
        )),
        area,
    );
}

fn truncate_for_preview(text: &str, max_chars: usize) -> String {
    let total_chars = text.chars().count();
    if total_chars <= max_chars {
        return text.to_string();
    }
    if max_chars <= 3 {
        return "...".chars().take(max_chars).collect();
    }
    let visible: String = text.chars().take(max_chars - 3).collect();
    format!("{}...", visible)
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

struct DashboardStats {
    total_sessions: usize,
    total_messages: usize,
    total_attachments: usize,
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_cost_usd: f64,
    today_sessions: usize,
    today_messages: usize,
    today_attachments: usize,
    today_input_tokens: u64,
    today_output_tokens: u64,
    today_cost_usd: f64,
    today_claude: usize,
    today_codex: usize,
    max_tool_count: usize,
    max_folder_count: usize,
    top_folders: Vec<(String, usize)>,
}

fn parse_local_date(ts: &Option<String>) -> Option<chrono::NaiveDate> {
    let ts_str = ts.as_deref()?;
    let utc = ts_str.parse::<DateTime<Utc>>().ok()?;
    Some(utc.with_timezone(&Local).date_naive())
}

fn compute_dashboard_stats(app: &App) -> DashboardStats {
    let today = Local::now().date_naive();
    let mut total_messages = 0usize;
    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;
    let mut total_cost_usd = 0.0f64;
    let mut today_sessions = 0usize;
    let mut today_messages = 0usize;
    let mut total_attachments = 0usize;
    let mut today_attachments = 0usize;
    let mut today_input_tokens = 0u64;
    let mut today_output_tokens = 0u64;
    let mut today_cost_usd = 0.0f64;
    let mut today_claude = 0usize;
    let mut today_codex = 0usize;
    let mut today_folders: HashMap<String, usize> = HashMap::new();

    for session in &app.sessions {
        total_messages += session.message_count;
        total_attachments += session.attachment_count;
        total_input_tokens = total_input_tokens.saturating_add(session.input_tokens);
        total_output_tokens = total_output_tokens.saturating_add(session.output_tokens);
        total_cost_usd += session.total_cost_usd;
        if parse_local_date(&session.timestamp) == Some(today) {
            today_sessions += 1;
            today_messages += session.message_count;
            today_attachments += session.attachment_count;
            today_input_tokens = today_input_tokens.saturating_add(session.input_tokens);
            today_output_tokens = today_output_tokens.saturating_add(session.output_tokens);
            today_cost_usd += session.total_cost_usd;
            match session.tool {
                CliTool::Claude => today_claude += 1,
                CliTool::Codex => today_codex += 1,
            }
            let folder = session
                .relative_folder
                .as_deref()
                .unwrap_or("root")
                .to_string();
            *today_folders.entry(folder).or_insert(0) += 1;
        }
    }

    let mut top_folders: Vec<(String, usize)> = today_folders.into_iter().collect();
    top_folders.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_folders.truncate(5);

    DashboardStats {
        total_sessions: app.sessions.len(),
        total_messages,
        total_attachments,
        total_input_tokens,
        total_output_tokens,
        total_cost_usd,
        today_sessions,
        today_messages,
        today_attachments,
        today_input_tokens,
        today_output_tokens,
        today_cost_usd,
        today_claude,
        today_codex,
        max_tool_count: today_claude.max(today_codex).max(1),
        max_folder_count: top_folders.iter().map(|(_, n)| *n).max().unwrap_or(1),
        top_folders,
    }
}

fn format_compact_u64(value: u64) -> String {
    if value >= 1_000_000_000 {
        format!("{:.1}B", value as f64 / 1_000_000_000.0)
    } else if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn mini_bar(value: usize, max: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let safe_max = max.max(1);
    let filled = (value * width) / safe_max;
    let mut out = String::with_capacity(width);
    for i in 0..width {
        if i < filled {
            out.push('#');
        } else {
            out.push('-');
        }
    }
    out
}
