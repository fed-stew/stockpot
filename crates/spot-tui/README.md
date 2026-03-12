# spot-tui

Terminal user interface for [Spot](https://github.com/fed-stew/spot), a precision computer control system.

## Features

- **Rich Terminal UI** - Beautiful ratatui-based interface
- **Markdown Rendering** - Syntax-highlighted code blocks
- **Multi-Agent Support** - Switch between specialized AI agents
- **Session History** - Persistent conversations
- **Clipboard Integration** - Easy copy/paste support

## Installation

```bash
cargo install spot-tui
```

Or add as a dependency:

```toml
[dependencies]
spot-tui = "0.22.0"
```

## Usage

### As a Binary

```bash
# Run the TUI
spot-cli

# Or if installed via cargo install
spot-tui
```

### As a Library

```rust
use spot_tui::tui::run_tui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_tui().await
}
```

## Screenshots

```
┌─────────────────────────────────────────────────────────────┐
│  Spot AI Assistant                              [Agent: v]  │
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

This is part of the Spot workspace:

- **spot-core** - Core library with all business logic
- **spot-tui** (this crate) - Terminal user interface
- **spot-gui** - Graphical user interface (build from source)

## License

MIT - See [LICENSE](https://github.com/fed-stew/spot/blob/main/LICENSE)
