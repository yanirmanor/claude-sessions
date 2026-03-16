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

// --- Tab bar ---
const TAB_ACTIVE_FG: Color = Color::Rgb(120, 200, 255);
const TAB_INACTIVE_FG: Color = Color::Rgb(100, 100, 120);
const TAB_SEPARATOR: Color = Color::Rgb(70, 70, 90);

// --- Content text ---
const TEXT_PRIMARY: Color = Color::Rgb(220, 220, 230);
const TEXT_MUTED: Color = Color::Rgb(110, 110, 130);
const TEXT_DIM: Color = Color::Rgb(70, 70, 85);
const HIGHLIGHT_BG: Color = Color::Rgb(40, 45, 75);
const HEADER_ROW_BG: Color = Color::Rgb(35, 35, 55);

// --- Semantic ---
const CLAUDE_COLOR: Color = Color::Rgb(200, 130, 255);
const CODEX_COLOR: Color = Color::Rgb(100, 220, 130);
const COST_COLOR: Color = Color::Rgb(255, 200, 80);
const BRANCH_COLOR: Color = Color::Cyan;
const FOLDER_COLOR: Color = Color::LightCyan;
const ATTACHMENT_COLOR: Color = Color::Magenta;
const SEPARATOR_COLOR: Color = Color::Rgb(50, 50, 65);

// --- Bars ---
const BAR_INPUT: Color = Color::Rgb(80, 180, 255);
const BAR_OUTPUT: Color = Color::Rgb(100, 220, 160);
const BAR_BG: Color = Color::Rgb(40, 40, 55);

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    let show_col_headers =
        app.screen == Screen::Sessions && !app.view_rows.is_empty();

    let chunks = Layout::vertical([
        Constraint::Length(1), // tab bar
        Constraint::Length(1), // search / context line
        Constraint::Length(if show_col_headers { 1 } else { 0 }), // column headers
        Constraint::Min(5),   // main content
        Constraint::Length(1), // footer
    ])
    .split(area);

    render_header(frame, app, chunks[0]);
    render_top_line(frame, app, chunks[1]);

    if show_col_headers {
        render_column_headers(frame, chunks[2]);
    }

    match app.screen {
        Screen::Sessions => render_sessions_screen(frame, app, chunks[3]),
        Screen::Stats => render_stats_screen(frame, app, chunks[3]),
        Screen::Skills => render_skills_screen(frame, app, chunks[3]),
    }

    render_footer(frame, app, chunks[4]);
}

fn render_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let tabs = [
        ("Sessions", Screen::Sessions),
        ("Stats", Screen::Stats),
        ("Skills", Screen::Skills),
    ];

    let mut left_spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, (label, screen)) in tabs.iter().enumerate() {
        if i > 0 {
            left_spans.push(Span::styled(" | ", Style::default().fg(TAB_SEPARATOR)));
        }
        let style = if app.screen == *screen {
            Style::default()
                .fg(TAB_ACTIVE_FG)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(TAB_INACTIVE_FG)
        };
        left_spans.push(Span::styled(*label, style));
    }

    let session_count = app.filtered_indices.len();
    let total = app.sessions.len();
    let count_text = if session_count == total {
        format!("({})", total)
    } else {
        format!("({}/{})", session_count, total)
    };

    let project_name = app
        .project_path
        .rsplit('/')
        .next()
        .unwrap_or(&app.project_path);

    let right_text = format!("{}  {} ", project_name, count_text);
    let left_len: usize = left_spans.iter().map(|s| s.content.len()).sum();
    let pad = (area.width as usize).saturating_sub(left_len + right_text.len());

    left_spans.push(Span::raw(" ".repeat(pad)));
    left_spans.push(Span::styled(
        project_name.to_string(),
        Style::default().fg(TEXT_MUTED),
    ));
    left_spans.push(Span::raw("  "));
    left_spans.push(Span::styled(count_text, Style::default().fg(COST_COLOR)));
    left_spans.push(Span::raw(" "));

    if app.screen == Screen::Sessions && app.attachments_only {
        left_spans.push(Span::styled(
            "[att]",
            Style::default().fg(ATTACHMENT_COLOR),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(left_spans)), area);
}

fn render_top_line(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    match app.screen {
        Screen::Sessions => {
            if app.view_rows.is_empty() {
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            " No sessions visible. ",
                            Style::default().fg(COST_COLOR),
                        ),
                        Span::styled(
                            "Try Esc to clear search or press 'a' to disable attachments filter.",
                            Style::default().fg(TEXT_DIM),
                        ),
                    ])),
                    area,
                );
                return;
            }

            let search_style = match app.mode {
                Mode::Search => Style::default().fg(COST_COLOR),
                Mode::Normal => Style::default().fg(TEXT_DIM),
            };
            let cursor_char = match app.mode {
                Mode::Search => "|",
                Mode::Normal => "",
            };
            let search = Paragraph::new(Line::from(vec![
                Span::styled(" Search: ", search_style),
                Span::styled(&app.search_query, Style::default().fg(TEXT_PRIMARY)),
                Span::styled(cursor_char, Style::default().fg(COST_COLOR)),
                Span::styled(
                    format!("  rows:{}", app.view_rows.len()),
                    Style::default().fg(TEXT_MUTED),
                ),
            ]));
            frame.render_widget(search, area);
        }
        Screen::Stats => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    " Stats dashboard",
                    Style::default().fg(TEXT_DIM),
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
                    Span::styled(" Skills filter: ", Style::default().fg(TEXT_DIM)),
                    Span::styled(filter_label, Style::default().fg(COST_COLOR)),
                    Span::styled(
                        "  (space toggle, e/d one, E/D all, g/p/a filter, r refresh)",
                        Style::default().fg(TEXT_DIM),
                    ),
                ])),
                area,
            );
        }
    }
}

fn render_column_headers(frame: &mut Frame, area: ratatui::layout::Rect) {
    let w = area.width as usize;
    let wide = w >= 100;
    let mid = w >= 80;

    let mut spans: Vec<Span> = vec![
        Span::styled(format!("  {:<14}", "when"), Style::default().fg(TEXT_DIM)),
        Span::styled(format!("{:<8}", "agent"), Style::default().fg(TEXT_DIM)),
    ];
    if mid {
        spans.push(Span::styled(
            format!("{:<16}", "branch"),
            Style::default().fg(TEXT_DIM),
        ));
    }
    spans.push(Span::styled(
        format!("{:>5} ", "msgs"),
        Style::default().fg(TEXT_DIM),
    ));
    if wide {
        spans.push(Span::styled(
            format!("{:<12}", "load"),
            Style::default().fg(TEXT_DIM),
        ));
        spans.push(Span::styled(
            format!("{:>7}", "in"),
            Style::default().fg(TEXT_DIM),
        ));
        spans.push(Span::styled(
            format!("{:>7}", "out"),
            Style::default().fg(TEXT_DIM),
        ));
        spans.push(Span::styled(
            format!("{:>7} ", "cost"),
            Style::default().fg(TEXT_DIM),
        ));
    }
    spans.push(Span::styled(
        " message",
        Style::default().fg(TEXT_DIM),
    ));

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(HEADER_ROW_BG)),
        area,
    );
}

fn render_sessions_screen(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    if app.view_rows.is_empty() {
        let mut lines = vec![Line::from(vec![Span::styled(
            " No sessions to display",
            Style::default()
                .fg(COST_COLOR)
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
                lines.push(Line::from(format!(
                    " - {}",
                    truncate_for_preview(file, 90)
                )));
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
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::TOP).title(" Sessions ")),
            area,
        );
        return;
    }

    let w = area.width as usize;
    let wide = w >= 100;
    let mid = w >= 80;
    let max_tokens = app.max_session_tokens;

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
                        Style::default().fg(COST_COLOR),
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
                ListItem::new(vec![separator, folder_line])
            }
            ViewRow::Session { session_idx, depth } => {
                let session = &app.sessions[*session_idx];
                let date_str = format_timestamp(&session.timestamp);
                let branch = session.git_branch.as_deref().unwrap_or("?");
                let is_empty = session.message_count == 0
                    && session.timestamp.is_none()
                    && session.git_branch.is_none();

                let (tool_label, tool_color) = match session.tool {
                    CliTool::Claude => ("Claude", CLAUDE_COLOR),
                    CliTool::Codex => ("Codex", CODEX_COLOR),
                };

                let indent_prefix = "  ".repeat(*depth);
                let dim = is_empty;
                let date_color = if dim { TEXT_DIM } else { TEXT_MUTED };
                let branch_color = if dim { TEXT_DIM } else { BRANCH_COLOR };
                let msg_count_color = if dim { TEXT_DIM } else { TEXT_PRIMARY };
                let tool_style = if dim {
                    Style::default().fg(TEXT_DIM)
                } else {
                    Style::default()
                        .fg(tool_color)
                        .add_modifier(Modifier::BOLD)
                };

                let mut spans: Vec<Span> = vec![
                    Span::styled(
                        format!(" {}{:<14}", indent_prefix, date_str),
                        Style::default().fg(date_color),
                    ),
                    Span::styled(format!("{:<8}", tool_label), tool_style),
                ];

                if mid {
                    let br = truncate_for_preview(branch, 14);
                    spans.push(Span::styled(
                        format!("{:<16}", br),
                        Style::default()
                            .fg(branch_color)
                            .add_modifier(if dim {
                                Modifier::empty()
                            } else {
                                Modifier::BOLD
                            }),
                    ));
                }

                spans.push(Span::styled(
                    format!("{:>5} ", session.message_count),
                    Style::default().fg(msg_count_color),
                ));

                if wide {
                    let bar_spans =
                        token_bar(session.input_tokens, session.output_tokens, max_tokens, 10);
                    spans.extend(bar_spans);
                    spans.push(Span::raw(" "));

                    spans.push(Span::styled(
                        format!("{:>7}", format_compact_u64(session.input_tokens)),
                        Style::default().fg(BAR_INPUT),
                    ));
                    spans.push(Span::styled(
                        format!("{:>7}", format_compact_u64(session.output_tokens)),
                        Style::default().fg(BAR_OUTPUT),
                    ));
                    spans.push(Span::styled(
                        format!("{:>7} ", format_cost(session.total_cost_usd)),
                        Style::default().fg(COST_COLOR),
                    ));
                }

                let used: usize = spans.iter().map(|s| s.content.len()).sum();
                let remaining = w.saturating_sub(used + 1);
                if remaining > 5 {
                    let msg_style = if session.first_user_message == "(no message)" || dim {
                        Style::default()
                            .fg(TEXT_DIM)
                            .add_modifier(Modifier::ITALIC)
                    } else {
                        Style::default().fg(TEXT_MUTED)
                    };
                    spans.push(Span::styled(
                        format!(" {}", truncate_for_preview(&session.first_user_message, remaining)),
                        msg_style,
                    ));
                }

                ListItem::new(vec![Line::from(spans)])
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default())
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
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                " Today",
                Style::default()
                    .fg(TAB_ACTIVE_FG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("{} sessions", stats.today_sessions),
                Style::default().fg(COST_COLOR),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Messages ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                format!("{}", stats.today_messages),
                Style::default().fg(TEXT_PRIMARY),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Tokens   ", Style::default().fg(TEXT_DIM)),
            Span::styled("in ", Style::default().fg(TEXT_MUTED)),
            Span::styled(
                format_compact_u64(stats.today_input_tokens),
                Style::default().fg(BAR_INPUT),
            ),
            Span::styled("  out ", Style::default().fg(TEXT_MUTED)),
            Span::styled(
                format_compact_u64(stats.today_output_tokens),
                Style::default().fg(BAR_OUTPUT),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Cost     ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                format!("${:.2}", stats.today_cost_usd),
                Style::default().fg(COST_COLOR),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Attach   ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                format!("{}", stats.today_attachments),
                Style::default().fg(TEXT_PRIMARY),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Tool Mix", Style::default().add_modifier(Modifier::BOLD)),
        ]),
    ];

    // Tool mix bars
    let max_tool = stats.max_tool_count;
    let mut claude_line: Vec<Span> = vec![
        Span::styled(format!(" Claude {:>3} ", stats.today_claude), Style::default().fg(CLAUDE_COLOR)),
    ];
    claude_line.extend(styled_bar(stats.today_claude, max_tool, 20, CLAUDE_COLOR));
    lines.push(Line::from(claude_line));

    let mut codex_line: Vec<Span> = vec![
        Span::styled(format!(" Codex  {:>3} ", stats.today_codex), Style::default().fg(CODEX_COLOR)),
    ];
    codex_line.extend(styled_bar(stats.today_codex, max_tool, 20, CODEX_COLOR));
    lines.push(Line::from(codex_line));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " All Time",
        Style::default()
            .fg(TAB_ACTIVE_FG)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled(" Sessions ", Style::default().fg(TEXT_DIM)),
        Span::styled(
            format!("{}", stats.total_sessions),
            Style::default().fg(TEXT_PRIMARY),
        ),
        Span::styled("  Messages ", Style::default().fg(TEXT_DIM)),
        Span::styled(
            format!("{}", stats.total_messages),
            Style::default().fg(TEXT_PRIMARY),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Tokens   ", Style::default().fg(TEXT_DIM)),
        Span::styled("in ", Style::default().fg(TEXT_MUTED)),
        Span::styled(
            format_compact_u64(stats.total_input_tokens),
            Style::default().fg(BAR_INPUT),
        ),
        Span::styled("  out ", Style::default().fg(TEXT_MUTED)),
        Span::styled(
            format_compact_u64(stats.total_output_tokens),
            Style::default().fg(BAR_OUTPUT),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Cost     ", Style::default().fg(TEXT_DIM)),
        Span::styled(
            format!("${:.2}", stats.total_cost_usd),
            Style::default().fg(COST_COLOR),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Attach   ", Style::default().fg(TEXT_DIM)),
        Span::styled(
            format!("{}", stats.total_attachments),
            Style::default().fg(TEXT_PRIMARY),
        ),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        " Top Folders (Today)",
        Style::default().add_modifier(Modifier::BOLD),
    )]));

    if stats.top_folders.is_empty() {
        lines.push(Line::from(Span::styled(
            " none",
            Style::default().fg(TEXT_DIM),
        )));
    } else {
        for (folder, count) in &stats.top_folders {
            let mut row: Vec<Span> = vec![
                Span::styled(" ", Style::default()),
                Span::styled(
                    format!("{:<32}", truncate_for_preview(folder, 30)),
                    Style::default().fg(FOLDER_COLOR),
                ),
                Span::styled(format!("{:>3} ", count), Style::default().fg(TEXT_PRIMARY)),
            ];
            row.extend(styled_bar(*count, stats.max_folder_count, 16, FOLDER_COLOR));
            lines.push(Line::from(row));
        }
    }

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP).title(" Stats ")),
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
                SkillScope::Global => CODEX_COLOR,
                SkillScope::Project => BRANCH_COLOR,
            };
            let status = if skill.has_skill_md {
                "ok"
            } else {
                "missing SKILL.md"
            };
            let enabled_text = if enabled { "ON" } else { "OFF" };
            let enabled_color = if enabled { CODEX_COLOR } else { Color::Red };
            ListItem::new(vec![Line::from(vec![
                Span::styled(
                    format!(" {} ", scope_badge),
                    Style::default()
                        .fg(scope_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    skill.name.clone(),
                    Style::default()
                        .fg(TEXT_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", enabled_text),
                    Style::default()
                        .fg(enabled_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {}", status), Style::default().fg(TEXT_DIM)),
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
                Span::styled(" Name: ", Style::default().fg(TEXT_DIM)),
                Span::styled(&skill.name, Style::default().fg(TEXT_PRIMARY)),
            ]),
            Line::from(vec![
                Span::styled(" Scope: ", Style::default().fg(TEXT_DIM)),
                Span::styled(scope, Style::default().fg(COST_COLOR)),
            ]),
            Line::from(vec![
                Span::styled(" Path: ", Style::default().fg(TEXT_DIM)),
                Span::styled(
                    skill.path.to_string_lossy().to_string(),
                    Style::default().fg(TEXT_PRIMARY),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Root: ", Style::default().fg(TEXT_DIM)),
                Span::styled(
                    skill.source_root.to_string_lossy().to_string(),
                    Style::default().fg(TEXT_PRIMARY),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Status: ", Style::default().fg(TEXT_DIM)),
                Span::styled(
                    if skill.has_skill_md {
                        "Ready"
                    } else {
                        "Missing SKILL.md"
                    },
                    Style::default().fg(if skill.has_skill_md {
                        CODEX_COLOR
                    } else {
                        Color::Red
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled(" Use in project: ", Style::default().fg(TEXT_DIM)),
                Span::styled(
                    if enabled { "Enabled" } else { "Disabled" },
                    Style::default().fg(if enabled { CODEX_COLOR } else { Color::Red }),
                ),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " No skills found for this filter",
            Style::default().fg(TEXT_DIM),
        ))]
    };

    frame.render_widget(
        Paragraph::new(details_lines)
            .block(Block::default().borders(Borders::TOP).title(" Details ")),
        chunks[1],
    );
}

fn render_footer(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let bindings: Vec<(&str, &str)> = match app.screen {
        Screen::Sessions => match app.mode {
            Mode::Normal => {
                if app.view_rows.is_empty() {
                    vec![
                        ("Esc", "clear"),
                        ("a", "attachments"),
                        ("R", "refresh"),
                        ("Tab", "next"),
                        ("q", "quit"),
                    ]
                } else {
                    vec![
                        ("j/k", "navigate"),
                        ("</>", "fold"),
                        ("[/]", "all"),
                        ("a", "attachments"),
                        ("/", "search"),
                        ("Enter", "resume"),
                        ("R", "refresh"),
                        ("Tab", "next"),
                        ("q", "quit"),
                    ]
                }
            }
            Mode::Search => vec![
                ("Up/Dn", "navigate"),
                ("Enter", "resume"),
                ("Esc", "clear"),
                ("Tab", "next"),
            ],
        },
        Screen::Stats => vec![("Tab", "next"), ("q", "quit")],
        Screen::Skills => vec![
            ("j/k", "navigate"),
            ("Space", "toggle"),
            ("e/d", "one"),
            ("E/D", "all"),
            ("g/p/a", "filter"),
            ("r", "refresh"),
            ("Tab", "next"),
            ("q", "quit"),
        ],
    };

    let mut spans: Vec<Span> = vec![Span::raw(" ")];
    for (i, (key, action)) in bindings.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default()));
        }
        spans.push(Span::styled(
            *key,
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {}", action),
            Style::default().fg(TEXT_DIM),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// --- Helpers ---

fn token_bar<'a>(input: u64, output: u64, max_total: u64, width: usize) -> Vec<Span<'a>> {
    let total = input.saturating_add(output);
    if total == 0 || max_total == 0 {
        return vec![Span::styled(
            "\u{2591}".repeat(width),
            Style::default().fg(BAR_BG),
        )];
    }
    let safe_max = max_total.max(1);
    let filled = ((total as f64 / safe_max as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    let input_cells = if total > 0 {
        ((input as f64 / total as f64) * filled as f64).round() as usize
    } else {
        0
    };
    let input_cells = input_cells.min(filled);
    let output_cells = filled.saturating_sub(input_cells);
    let empty = width.saturating_sub(filled);

    let mut spans = Vec::new();
    if input_cells > 0 {
        spans.push(Span::styled(
            "\u{2588}".repeat(input_cells),
            Style::default().fg(BAR_INPUT),
        ));
    }
    if output_cells > 0 {
        spans.push(Span::styled(
            "\u{2588}".repeat(output_cells),
            Style::default().fg(BAR_OUTPUT),
        ));
    }
    if empty > 0 {
        spans.push(Span::styled(
            "\u{2591}".repeat(empty),
            Style::default().fg(BAR_BG),
        ));
    }
    spans
}

fn styled_bar<'a>(value: usize, max: usize, width: usize, fg: Color) -> Vec<Span<'a>> {
    if width == 0 {
        return Vec::new();
    }
    let safe_max = max.max(1);
    let filled = (value * width) / safe_max;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);
    let mut spans = Vec::new();
    if filled > 0 {
        spans.push(Span::styled(
            "\u{2588}".repeat(filled),
            Style::default().fg(fg),
        ));
    }
    if empty > 0 {
        spans.push(Span::styled(
            "\u{2591}".repeat(empty),
            Style::default().fg(BAR_BG),
        ));
    }
    spans
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
                local_dt.format("%m/%d %H:%M").to_string()
            } else {
                "  --  --:--".to_string()
            }
        }
        None => "  --  --:--".to_string(),
    }
}

fn format_cost(cost: f64) -> String {
    if cost >= 10.0 {
        format!("${:.1}", cost)
    } else if cost >= 0.01 {
        format!("${:.2}", cost)
    } else if cost > 0.0 {
        "$<.01".to_string()
    } else {
        "$0".to_string()
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
