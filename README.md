# Stockpot

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**Your AI-powered coding companion** — A blazing-fast, terminal-native coding assistant built in Rust.

Stockpot is an open-source alternative to expensive AI coding tools like Cursor and Windsurf. It runs directly in your terminal with support for multiple LLM providers, tool execution, and a delightful developer experience.

## ✨ Features

### 🤖 Multi-Provider AI Support
- **OpenAI**: GPT-4o, GPT-4o-mini, O1, O1-mini
- **Anthropic**: Claude 3.5 Sonnet, Claude 3.5 Haiku, Claude 3 Opus
- **Google**: Gemini 2.0 Flash, Gemini 1.5 Pro
- **OAuth Providers**: ChatGPT Plus, Claude Code (VS Code credentials)

### 🛠️ Powerful Tools
- **File Operations**: Read, write, list, grep with smart filtering
- **Shell Commands**: Execute with streaming output and timeout handling
- **Diff Application**: Proper unified diff parsing and patching
- **Syntax Highlighting**: Rich markdown rendering with syntect

### 🎯 Agent System
- **Built-in Agents**: Stockpot, Planner, Language-specific Reviewers
- **Custom JSON Agents**: Define your own agents with custom prompts
- **Sub-Agent Invocation**: Agents can delegate to specialized agents
- **Capability Controls**: Fine-grained permissions per agent

### 🔌 MCP Integration
- **Model Context Protocol**: Connect to MCP servers for extended tools
- **Auto-Discovery**: Load tools from filesystem, GitHub, and custom servers
- **Hot-Reload**: Add/remove servers without restarting

### 💾 Session Management
- **Save/Load Sessions**: Persist conversations for later
- **Context Control**: Truncate, pin models, manage history
- **Auto-Cleanup**: Smart session retention

### 🎨 Developer Experience
- **Tab Completion**: Commands, models, agents, sessions
- **Animated Spinner**: Activity indicator during LLM calls
- **Rich Output**: Markdown, code blocks, diffs with colors
- **Bridge Mode**: NDJSON protocol for external UI integration

## 📦 Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/Fed-Stew/stockpot
cd stockpot

# Build and install
cargo install --path .

# Or build in release mode
cargo build --release
./target/release/pup
```

### Prerequisites
- Rust 1.75 or later
- ripgrep (`rg`) for fast searching

## 🚀 Quick Start

### Set up your API key

```bash
# OpenAI
export OPENAI_API_KEY="sk-..."

# Or Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# Or Google
export GOOGLE_API_KEY="..."
```

### Start Stockpot

```bash
# Interactive mode
pup

# Single prompt
pup -p "Explain this codebase"

# With specific agent
pup --agent python-reviewer

# With specific model
pup --model anthropic:claude-3-5-sonnet
```

### OAuth Authentication (ChatGPT/Claude Code)

```bash
# Inside the REPL
/chatgpt-auth      # For ChatGPT Plus
/claude-code-auth  # For Claude Code (uses VS Code credentials)
```

## 📋 Commands

### Navigation
| Command | Description |
|---------|-------------|
| `/help` | Show all commands |
| `/exit` | Exit Stockpot |
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

## ⚙️ Configuration

### Config Files

```
~/.stockpot/
├── config.db          # SQLite database (settings, tokens)
├── sessions/          # Saved conversation sessions
│   └── *.json
├── agents/            # Custom JSON agents
│   └── my-agent.json
└── mcp.json           # MCP server configuration
```

### Custom Agents (`~/.stockpot/agents/*.json`)

```json
{
  "name": "my-agent",
  "display_name": "My Agent 🤖",
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

### MCP Configuration (`~/.stockpot/mcp.json`)

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

## 🌐 Bridge Mode

For external UI integration (VS Code extension, web UI, etc.):

```bash
pup --bridge
```

Communicates via NDJSON over stdio:

```json
// → Outbound
{"type": "ready", "version": "0.1.0", "agent": "stockpot", "model": "gpt-4o"}
{"type": "text_delta", "text": "Hello..."}
{"type": "tool_call_start", "tool_name": "read_file"}
{"type": "complete", "run_id": "..."}

// ← Inbound
{"type": "prompt", "text": "Help me code"}
{"type": "cancel"}
{"type": "shutdown"}
```

## 🧪 Development

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

## 📚 Architecture

```
src/
├── agents/           # Agent system
│   ├── base.rs       # PuppyAgent trait
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

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [serdesAI](https://github.com/janfeddersen-wq/serdesAI) - AI agent framework
- Inspired by [Claude Code](https://anthropic.com) and [Cursor](https://cursor.so)
- Terminal UI powered by [crossterm](https://github.com/crossterm-rs/crossterm) and [rustyline](https://github.com/kkawakam/rustyline)
- Syntax highlighting by [syntect](https://github.com/trishume/syntect)

---

**Made with ❤️ by the Fed Stew team**
