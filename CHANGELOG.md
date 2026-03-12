# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.24.0] - 2026-03-12

### Added
- **GUI**: VDI mode with auto-detection for Citrix, VMware Horizon, RDP, and Amazon WorkSpaces
  - Reduces animation frame rate from 120fps to ~15fps in VDI environments
  - Toggle in Settings > General (shows "auto-detected" when VDI is detected)
  - Configurable frame interval via `vdi.frame_interval_ms` setting
- **Core**: `detect_vdi_environment()` and `is_vdi_mode_active()` for VDI detection
- **GUI**: Configurable spinner timer interval for VDI mode (`Spinner::with_interval()`)

### Changed
- **GUI**: Animation loop only triggers re-renders when content actually changes
  - Scroll moved, pending text updates, or explicit render flag required
  - Decoupled data updates (throughput, scroll physics) from render triggers
- **CI**: Pipeline restructured as sequential gate: fmt -> clippy -> tests
  - Removed redundant cargo check job (clippy already covers it)
  - Saves runner time by failing fast on formatting/lint issues
- **Release**: Pipeline restructured with same fmt -> clippy -> tests gate before multi-platform build
- Renamed crates from `stockpot-*` to `spot-*`

## [0.23.4] - 2025-01-27

### Added
- **TUI**: OAuth authentication dialog with copyable URL for SSH/headless users
  - Modal dialog shows auth URL prominently for easy copy/paste
  - Shows callback port for SSH port forwarding: `ssh -L PORT:localhost:PORT`
  - Auto-closes on successful authentication
  - Escape key dismisses dialog

### Changed  
- **Core**: `AuthProgress::on_auth_url()` callback for capturing URL/port before waiting

## [0.23.3] - 2025-01-27

### Added
- **TUI**: OAuth authentication now fully supported! Connect to ChatGPT, Claude Code, and Google directly from TUI
  - Navigate OAuth providers with ↑/↓ arrow keys in Settings > Models
  - Press Enter to start OAuth flow - browser opens automatically
  - Progress messages display in activity feed (no more stdout corruption)
  - Models auto-refresh on successful authentication
- **Core**: New `AuthProgress` trait for customizable OAuth progress reporting
  - `run_*_auth_with_progress()` functions for all OAuth providers
  - `StdoutProgress` default implementation for CLI
  - `MessageBusProgress` TUI implementation routes to activity feed

## [0.23.2] - 2025-01-27

### Fixed
- **TUI**: Arrow keys no longer double-fire on Windows in dropdown menus and folder selection
  - Added `KeyEventKind::Press` filter to prevent processing Release/Repeat events
  - Windows crossterm fires multiple event kinds for arrow keys; macOS only fires Press

## [0.23.0] - 2025-01-27

### Added
- **TUI**: 3-line default input bar for more comfortable prompt typing (expandable to 5 lines)
- **TUI**: Visual scrollbar on message container showing position in conversation
- **TUI**: Per-model settings (temperature, top_p) in Settings > Models tab - press Enter to expand a model
- **TUI**: Default model dropdown in Settings > Pinned Agents tab with modal overlay
- **TUI**: Up/Down arrow navigation within expanded model settings panel

### Fixed
- **TUI**: 'k' key now correctly opens API key pool management for expanded models
- **TUI**: Escape key properly closes default model dropdown before closing settings

## [0.22.3] - 2025-01-26

### Fixed
- Terminal character grid rendering now uses fixed-width cells for proper TUI app alignment (htop, vim, etc.)
- Default terminal size increased from 16×50 to 24×120 for better compatibility with modern TUI applications

## [0.22.2] - 2025-01-26

### Fixed
- CI workflows updated for workspace structure
- Doctests updated to use `spot_core` crate name
- Clippy warnings and formatting issues resolved

## [0.22.1] - 2025-01-26

### Added
- README.md files for spot-core and spot-tui crates

## [0.22.0] - 2025-01-26

### Changed
- **BREAKING**: Split into workspace with 3 crates:
  - `spot-core` - Core library (publishable to crates.io)
  - `spot-tui` - Terminal UI (publishable to crates.io)
  - `spot-gui` - GUI (git-only due to gpui dependency)
- Updated streamdown dependencies to crates.io versions (0.1.4)

### Added
- Lenient JSON argument parsing for tool calls - automatically coerces "almost correct" LLM responses
- New utility functions: `coerce_json_types()` and `parse_tool_args_lenient()`
- 24 new unit tests for JSON coercion

### Fixed
- LLM tool calls no longer fail when types are semantically correct but technically wrong
