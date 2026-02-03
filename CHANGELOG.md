# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- Doctests updated to use `stockpot_core` crate name
- Clippy warnings and formatting issues resolved

## [0.22.1] - 2025-01-26

### Added
- README.md files for stockpot-core and stockpot-tui crates

## [0.22.0] - 2025-01-26

### Changed
- **BREAKING**: Split into workspace with 3 crates:
  - `stockpot-core` - Core library (publishable to crates.io)
  - `stockpot-tui` - Terminal UI (publishable to crates.io)
  - `stockpot-gui` - GUI (git-only due to gpui dependency)
- Updated streamdown dependencies to crates.io versions (0.1.4)

### Added
- Lenient JSON argument parsing for tool calls - automatically coerces "almost correct" LLM responses
- New utility functions: `coerce_json_types()` and `parse_tool_args_lenient()`
- 24 new unit tests for JSON coercion

### Fixed
- LLM tool calls no longer fail when types are semantically correct but technically wrong
