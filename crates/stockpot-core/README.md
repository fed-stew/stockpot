# stockpot-core

Core library for [Stockpot](https://github.com/fed-stew/stockpot), an AI-powered coding assistant.

## Features

- **AI Agent Framework** - Build and orchestrate AI coding agents
- **Tool System** - File operations, grep, shell commands, and more
- **Multi-Provider Support** - OpenAI, Anthropic, Google, local models
- **MCP Integration** - Model Context Protocol for extensibility
- **Session Management** - Persistent conversation history
- **Terminal Emulation** - Full PTY support for interactive commands

## Installation

```toml
[dependencies]
stockpot-core = "0.22.0"
```

## Usage

```rust
use stockpot_core::agents::manager::AgentManager;
use stockpot_core::config::Settings;
use stockpot_core::db::Database;

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

This is part of the Stockpot workspace:

- **stockpot-core** (this crate) - Core library with all business logic
- **stockpot-tui** - Terminal user interface
- **stockpot-gui** - Graphical user interface (build from source)

## License

MIT - See [LICENSE](https://github.com/fed-stew/stockpot/blob/main/LICENSE)
