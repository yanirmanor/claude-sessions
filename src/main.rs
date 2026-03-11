mod app;
mod fuzzy;
mod session;
mod ui;

use std::io;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{Action, App};
use session::{load_sessions, CliTool};

#[derive(Parser)]
#[command(name = "claude-sessions", about = "Browse and resume Claude Code and Codex CLI sessions")]
struct Cli {
    /// Project path (defaults to current directory)
    #[arg(short, long)]
    path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let project_path = cli
        .path
        .unwrap_or_else(|| std::env::current_dir().expect("Cannot get current directory"));

    let project_path_str = project_path.to_string_lossy().to_string();

    // Install panic hook that restores terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = io::stdout().execute(LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    let sessions = load_sessions(&project_path)?;
    if sessions.is_empty() {
        println!("No sessions found for {}", project_path.display());
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(sessions, project_path_str);

    // Event loop
    let resume_action = loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.handle_key(key) {
                Action::Quit => break None,
                Action::Resume(id, tool) => break Some((id, tool)),
                Action::None => {}
            }
        }
    };

    // Restore terminal
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;

    // Resume session if requested — uses Unix exec() to replace current process
    if let Some((session_id, tool)) = resume_action {
        let err = match tool {
            CliTool::Claude => Command::new("claude")
                .arg("--resume")
                .arg(&session_id)
                .exec(),
            CliTool::Codex => Command::new("codex")
                .arg("resume")
                .arg(&session_id)
                .exec(),
        };
        let tool_name = match tool {
            CliTool::Claude => "claude",
            CliTool::Codex => "codex",
        };
        eprintln!("Failed to launch {}: {}", tool_name, err);
        std::process::exit(1);
    }

    Ok(())
}
