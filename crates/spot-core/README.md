# spot-core

Core library for [Spot](https://github.com/fed-stew/spot), a precision computer control system.

## Features

- **AI Agent Framework** - Build and orchestrate AI agents
- **Tool System** - File operations, grep, shell commands, and more
- **Multi-Provider Support** - OpenAI, Anthropic, Google, local models
- **MCP Integration** - Model Context Protocol for extensibility
- **Session Management** - Persistent conversation history
- **Terminal Emulation** - Full PTY support for interactive commands

## Installation

```toml
[dependencies]
spot-core = "0.22.0"
```

## Usage

```rust
use spot_core::agents::manager::AgentManager;
use spot_core::config::Settings;
use spot_core::db::Database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize database
    let db = Database::open_default().await?;

    // Load settings
    let settings = Settings::load()?;

    // Create agent manager
    let manager = AgentManager::new(db, settings).await?;

    // Use agents...
    Ok(())
}
```

## Crate Structure

This is part of the Spot workspace:

- **spot-core** (this crate) - Core library with all business logic
- **spot-tui** - Terminal user interface
- **spot-gui** - Graphical user interface (build from source)

## License

MIT - See [LICENSE](https://github.com/fed-stew/spot/blob/main/LICENSE)
