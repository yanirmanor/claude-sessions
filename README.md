# claude-sessions

A TUI tool to browse and resume your [Claude Code](https://docs.anthropic.com/en/docs/claude-code) sessions.

![Rust](https://img.shields.io/badge/rust-stable-orange)

## Features

- Browse all Claude Code sessions for a project
- Fuzzy search to filter sessions
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

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) must be installed (`claude` CLI available in PATH)

## License

MIT
