# claude-sessions

A TUI tool to browse and resume your [Claude Code](https://docs.anthropic.com/en/docs/claude-code) and [Codex CLI](https://github.com/openai/codex) sessions from one interface.

![Rust](https://img.shields.io/badge/rust-stable-orange)

## Features

- Browse all Claude Code and Codex CLI sessions for a project
- Multi-tool support: sessions from both tools shown with `[Claude]` / `[Codex]` badges
- Codex sessions filtered by project working directory when possible
- Fuzzy search to filter sessions across all tools
- Expand/collapse truncated messages with arrow keys
- Resume any session directly from the TUI

## Install

### From source (requires [Rust](https://rustup.rs/))

```bash
git clone https://github.com/yanirmanor/claude-sessions.git
cd claude-sessions
cargo install --path .
```

### Build only

```bash
cargo build --release
# binary is at target/release/claude-sessions
```

## Usage

```bash
# Browse sessions for the current directory
claude-sessions

# Browse sessions for a specific project
claude-sessions --path /path/to/project
```

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` / `Up` / `Down` | Navigate sessions |
| `Right` | Expand selected message |
| `Left` | Collapse selected message |
| `Enter` | Resume selected session |
| `/` | Search |
| `Esc` | Exit search / Quit |
| `q` | Quit |

## Requirements

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) must be installed (`claude` CLI available in PATH) to resume Claude sessions
- [Codex CLI](https://github.com/openai/codex) must be installed (`codex` CLI available in PATH) to resume Codex sessions

## License

MIT
