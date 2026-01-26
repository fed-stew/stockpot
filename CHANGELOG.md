# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.21.9] - 2025-01-26

### Added
- Lenient JSON argument parsing for tool calls - automatically coerces "almost correct" LLM responses:
  - String `"true"`/`"false"` → boolean `true`/`false`
  - String `"42"` → integer `42`
  - String `"3.14"` → number `3.14`
- New utility functions in `tools/common.rs`: `coerce_json_types()` and `parse_tool_args_lenient()`
- 24 new unit tests for JSON coercion

### Changed
- All 9 built-in tools now use lenient argument parsing
- Reduced boilerplate in tool argument parsing code

### Fixed
- LLM tool calls no longer fail when types are semantically correct but technically wrong (e.g., `"true"` instead of `true`)
