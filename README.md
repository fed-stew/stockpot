# Spot 👁️

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**"Spot what matters. Control everything."** — A blazing-fast precision computer control system built in Rust with GUI, CLI, and TUI modes.

Spot is an open-source alternative to expensive AI coding tools like Cursor and Windsurf. It features a beautiful native GUI (default), a powerful terminal REPL, and bridge mode for IDE integration. Supports multiple LLM providers including OAuth authentication for ChatGPT Plus and Claude Code subscriptions.

## Features

### Multi-Provider AI Support
- **OpenAI**: GPT-4.1, GPT-4.1-mini, GPT-4.1-nano, GPT-4o, O3, O4-mini
- **Anthropic**: Claude Sonnet 4, Claude Opus 4, Claude 3.5 Haiku
- **Google**: Gemini 2.5 Pro, Gemini 2.5 Flash, Gemini 2.0 Flash
- **ChatGPT OAuth**: Use your ChatGPT Plus/Pro subscription directly (no API key needed!)
- **Claude Code OAuth**: Use your Claude Pro/Max subscription via VS Code credentials

### Native GUI (Default)
- **Beautiful Interface**: Modern, responsive design built with GPUI
- **Real-time Streaming**: Watch responses flow in with live markdown rendering
- **Model & Agent Switching**: Easy dropdowns for quick context switching
- **Settings Panel**: Configure everything visually
- **Dark Theme**: Easy on the eyes for long sessions

### Powerful Tools
- **File Operations**: Read, write, list, grep with smart filtering
- **Shell Commands**: Execute with streaming output and timeout handling
- **Diff Application**: Proper unified diff parsing and patching
- **Syntax Highlighting**: Rich markdown rendering with syntect

### Agent System
- **Built-in Agents**: Spot, Planner, Language-specific Reviewers
- **Custom JSON Agents**: Define your own agents with custom prompts
- **Sub-Agent Invocation**: Agents can delegate to specialized agents
- **Capability Controls**: Fine-grained permissions per agent

### MCP Integration
- **Model Context Protocol**: Connect to MCP servers for extended tools
- **Auto-Discovery**: Load tools from filesystem, GitHub, and custom servers
- **Hot-Reload**: Add/remove servers without restarting

### Session Management
- **Save/Load Sessions**: Persist conversations for later
- **Context Control**: Truncate, pin models, manage history
- **Auto-Cleanup**: Smart session retention

### Developer Experience
- **Tab Completion**: Commands, models, agents, sessions
- **Animated Spinner**: Activity indicator during LLM calls
- **Rich Output**: Markdown, code blocks, diffs with colors
- **Bridge Mode**: NDJSON protocol for external UI integration

## Installation

### From GitHub Releases (Recommended)

Download the latest release for your platform from the [Releases page](https://github.com/your-org/spot/releases).

**Linux (x86_64)**:
```bash
curl -LO https://github.com/your-org/spot/releases/latest/download/spot-linux-x86_64.tar.gz
tar xzf spot-linux-x86_64.tar.gz
sudo mv spot /usr/local/bin/
```

**macOS (Intel)**:
```bash
curl -LO https://github.com/your-org/spot/releases/latest/download/spot-macos-x86_64.tar.gz
tar xzf spot-macos-x86_64.tar.gz
sudo mv spot /usr/local/bin/
```

**macOS (Apple Silicon)**:
```bash
curl -LO https://github.com/your-org/spot/releases/latest/download/spot-macos-aarch64.tar.gz
tar xzf spot-macos-aarch64.tar.gz
sudo mv spot /usr/local/bin/
```

**Windows**:
Download `spot-windows-x86_64.zip` from the releases page, extract it, and add the directory to your PATH.

### From Source

```bash
git clone https://github.com/your-org/spot.git
cd spot
cargo install --path .
```

### Verify Installation

```bash
spot --version
```

### Prerequisites
- Rust 1.75 or later (only needed for building from source)

## Quick Start

### Set up your API key

```bash
# OpenAI
export OPENAI_API_KEY="sk-..."

# Or Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# Or Google
export GOOGLE_API_KEY="..."
```

### Start Spot

```bash
# Launch GUI (default)
spot

# Terminal REPL mode
spot --no-gui

# Single prompt (terminal)
spot --no-gui -p "Explain this codebase"

# With specific agent
spot --agent python-reviewer

# With specific model
spot --model anthropic:claude-3-5-sonnet
```

### OAuth Authentication (ChatGPT/Claude Code)

```bash
# Inside the REPL
/chatgpt-auth      # For ChatGPT Plus
/claude-code-auth  # For Claude Code (uses VS Code credentials)
```

## Commands

### Navigation
| Command | Description |
|---------|-------------|
| `/help` | Show all commands |
| `/exit` | Exit Spot |
| `/clear` | Clear the screen |
| `/new` | Start a new conversation |

### Agents & Models
| Command | Description |
|---------|-------------|
| `/model [name]` | Show or set the current model |
| `/models` | List all available models |
| `/agent [name]` | Show or switch to an agent |
| `/agents` | List all available agents |
| `/pin <model>` | Pin a model to the current agent |
| `/unpin` | Remove model pin |

### Sessions
| Command | Description |
|---------|-------------|
| `/save [name]` | Save current session |
| `/load [name]` | Load a session |
| `/sessions` | List saved sessions |
| `/delete-session <name>` | Delete a session |

### Context
| Command | Description |
|---------|-------------|
| `/context` | Show context usage info |
| `/truncate [n]` | Keep only last N messages |

### MCP
| Command | Description |
|---------|-------------|
| `/mcp status` | Show MCP server status |
| `/mcp start [name]` | Start MCP server(s) |
| `/mcp stop [name]` | Stop MCP server(s) |
| `/mcp tools [name]` | List tools from server |

### Settings
| Command | Description |
|---------|-------------|
| `/set [key=value]` | Show or set configuration |
| `/yolo` | Toggle YOLO mode (auto-approve) |

## Configuration

### Config Files

```
~/.spot/
├── config.db          # SQLite database (settings, tokens)
├── sessions/          # Saved conversation sessions
│   └── *.json
├── agents/            # Custom JSON agents
│   └── my-agent.json
└── mcp.json           # MCP server configuration
```

### User Modes & Agent Visibility

Spot organizes agents into three visibility levels to reduce clutter for different user experience levels:

| Visibility | Description |
|------------|-------------|
| **Main** | Core agents always visible (spot, planning) |
| **Sub** | Specialized agents for power users (reviewers, explore) |
| **Hidden** | Development/example agents |

#### Setting User Mode

Control which agents you see based on your experience level:

- **Normal** (default): Shows only Main agents - perfect for everyday use
- **Expert**: Shows Main + Sub agents - for power users who want specialized tools
- **Developer**: Shows all agents including Hidden - for development and debugging

```bash
# Set user mode via CLI
/set user_mode=expert

# Or use the GUI settings panel
```

#### Agent Visibility in JSON

Custom JSON agents can specify their visibility:

```json
{
  "name": "my-agent",
  "visibility": "sub"
}
```

### Custom Agents (`~/.spot/agents/*.json`)

```json
{
  "name": "my-agent",
  "display_name": "My Agent",
  "description": "A custom specialized agent",
  "system_prompt": "You are a helpful assistant specialized in...",
  "tools": ["read_file", "edit_file", "grep", "run_shell_command"],
  "model": "openai:gpt-4o",
  "capabilities": {
    "file_read": true,
    "file_write": true,
    "shell": true,
    "sub_agents": false
  }
}
```

### MCP Configuration (`~/.spot/mcp.json`)

```json
{
  "servers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-server-fs", "/home/user"],
      "enabled": true
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-server-github"],
      "env": {
        "GITHUB_TOKEN": "${GITHUB_TOKEN}"
      },
      "enabled": true
    }
  }
}
```

## Bridge Mode

For external UI integration (VS Code extension, web UI, etc.):

```bash
spot --bridge
```

Communicates via NDJSON over stdio:

```json
// Outbound
{"type": "ready", "version": "0.5.0", "agent": "spot", "model": "gpt-4o"}
{"type": "text_delta", "text": "Hello..."}
{"type": "tool_call_start", "tool_name": "read_file"}
{"type": "complete", "run_id": "..."}

// Inbound
{"type": "prompt", "text": "Help me code"}
{"type": "cancel"}
{"type": "shutdown"}
```

## Development

```bash
# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run

# Check for issues
cargo clippy

# Format code
cargo fmt
```

## Architecture

```
src/
├── agents/           # Agent system
│   ├── base.rs       # SpotAgent trait
│   ├── builtin/      # Built-in agents
│   ├── executor.rs   # Agent execution with streaming
│   ├── json_agent.rs # JSON agent loader
│   └── manager.rs    # Agent registry
├── auth/             # OAuth authentication
├── cli/              # CLI components
│   ├── bridge.rs     # Bridge mode (NDJSON)
│   ├── completion.rs # Tab completion
│   ├── repl.rs       # Interactive REPL
│   └── runner.rs     # CLI entry points
├── config/           # Configuration
├── db/               # SQLite database
├── mcp/              # MCP integration
├── messaging/        # UI messaging
│   ├── renderer.rs   # Markdown rendering
│   └── spinner.rs    # Activity spinner
├── session/          # Session management
└── tools/            # Tool implementations
    ├── diff.rs       # Unified diff parser
    ├── file_ops.rs   # File operations
    ├── registry.rs   # Tool registry
    └── shell.rs      # Shell execution
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [serdesAI](https://github.com/janfeddersen-wq/serdesAI) - AI agent framework
- Inspired by [Claude Code](https://anthropic.com) and [Cursor](https://cursor.so)
- Terminal UI powered by [crossterm](https://github.com/crossterm-rs/crossterm) and [rustyline](https://github.com/kkawakam/rustyline)
- Syntax highlighting by [syntect](https://github.com/trishume/syntect)

---

**Made with care by the Spot team**
