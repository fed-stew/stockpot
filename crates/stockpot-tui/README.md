# stockpot-tui

Terminal user interface for [Stockpot](https://github.com/fed-stew/stockpot), an AI-powered coding assistant.

## Features

- **Rich Terminal UI** - Beautiful ratatui-based interface
- **Markdown Rendering** - Syntax-highlighted code blocks
- **Multi-Agent Support** - Switch between specialized AI agents
- **Session History** - Persistent conversations
- **Clipboard Integration** - Easy copy/paste support

## Installation

```bash
cargo install stockpot-tui
```

Or add as a dependency:

```toml
[dependencies]
stockpot-tui = "0.22.0"
```

## Usage

### As a Binary

```bash
# Run the TUI
spot-cli

# Or if installed via cargo install
stockpot-tui
```

### As a Library

```rust
use stockpot_tui::tui::run_tui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_tui().await
}
```

## Screenshots

```
┌─────────────────────────────────────────────────────────────┐
│  Stockpot AI Assistant                          [Agent: ▼]  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  > How can I help you today?                                │
│                                                             │
│  User: Can you help me refactor this function?              │
│                                                             │
│  Assistant: I'd be happy to help! Let me take a look...     │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│  Type your message...                              [Enter]  │
└─────────────────────────────────────────────────────────────┘
```

## Crate Structure

This is part of the Stockpot workspace:

- **stockpot-core** - Core library with all business logic
- **stockpot-tui** (this crate) - Terminal user interface
- **stockpot-gui** - Graphical user interface (build from source)

## License

MIT - See [LICENSE](https://github.com/fed-stew/stockpot/blob/main/LICENSE)
