//! Common utilities for tools.

use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use serdes_ai_tools::ToolError;
use tracing::debug;

/// Directory patterns to ignore.
pub static IGNORE_PATTERNS: &[&str] = &[
    // Version control
    ".git",
    ".svn",
    ".hg",
    // Dependencies
    "node_modules",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    // Build outputs
    "target",
    "dist",
    "build",
    ".next",
    ".nuxt",
    // IDE/Editor
    ".idea",
    ".vscode",
    // Cache
    ".cache",
    ".pytest_cache",
    ".mypy_cache",
    // Package managers
    ".npm",
    ".yarn",
    ".pnpm-store",
];

/// Check if a path should be ignored.
pub fn should_ignore(path: &str) -> bool {
    let path_lower = path.to_lowercase();

    for pattern in IGNORE_PATTERNS {
        if path_lower.contains(pattern) {
            return true;
        }
    }

    false
}

/// Get file extension.
pub fn get_extension(path: &str) -> Option<&str> {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
}

/// Check if path is likely a text file.
pub fn is_text_file(path: &str) -> bool {
    let text_extensions = [
        "txt",
        "md",
        "rs",
        "py",
        "js",
        "ts",
        "tsx",
        "jsx",
        "json",
        "yaml",
        "yml",
        "toml",
        "ini",
        "cfg",
        "html",
        "css",
        "scss",
        "less",
        "sh",
        "bash",
        "zsh",
        "fish",
        "c",
        "h",
        "cpp",
        "hpp",
        "cc",
        "cxx",
        "go",
        "java",
        "kt",
        "swift",
        "rb",
        "php",
        "sql",
        "graphql",
        "proto",
        "xml",
        "svg",
        "dockerfile",
        "makefile",
    ];

    if let Some(ext) = get_extension(path) {
        text_extensions.contains(&ext.to_lowercase().as_str())
    } else {
        // No extension - check filename
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let text_files = [
            "Makefile",
            "Dockerfile",
            "Rakefile",
            "Gemfile",
            ".gitignore",
            ".env",
        ];
        text_files.contains(&name)
    }
}

/// Recursively coerces JSON values to match the expected types in a JSON Schema.
///
/// LLMs sometimes return "almost correct" JSON with wrong types like:
/// - `"true"` (string) instead of `true` (boolean)
/// - `"42"` (string) instead of `42` (integer)
/// - `"3.14"` (string) instead of `3.14` (number)
///
/// This function walks the schema and coerces mismatched types in `args`.
///
/// # Arguments
/// * `args` - The JSON value to coerce (modified in place)
/// * `schema` - The JSON Schema describing expected types
///
/// # Coercion Rules
/// - Schema type `boolean`: `"true"`/`"True"`/`"TRUE"`/`"1"` → `true`,
///   `"false"`/`"False"`/`"FALSE"`/`"0"` → `false`
/// - Schema type `integer`: `"42"` → `42` (parse string as i64)
/// - Schema type `number`: `"3.14"` → `3.14` (parse string as f64)
pub fn coerce_json_types(args: &mut JsonValue, schema: &JsonValue) {
    // Handle object schemas with "properties"
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        if let Some(args_obj) = args.as_object_mut() {
            for (key, prop_schema) in properties {
                if let Some(value) = args_obj.get_mut(key) {
                    coerce_value(value, prop_schema, key);
                }
            }
        }
        return;
    }

    // Handle direct type coercion (when schema is a simple type definition)
    if let Some(type_str) = schema.get("type").and_then(|t| t.as_str()) {
        coerce_value(args, schema, "root");
        let _ = type_str; // silence unused warning
    }
}

/// Coerces a single value based on its schema type.
fn coerce_value(value: &mut JsonValue, schema: &JsonValue, field_name: &str) {
    let Some(type_str) = schema.get("type").and_then(|t| t.as_str()) else {
        return;
    };

    match type_str {
        "boolean" => coerce_to_boolean(value, field_name),
        "integer" => coerce_to_integer(value, field_name),
        "number" => coerce_to_number(value, field_name),
        "object" => {
            // Recurse into nested objects
            coerce_json_types(value, schema);
        }
        "array" => {
            // Handle array items if schema specifies items
            if let (Some(items_schema), Some(arr)) = (schema.get("items"), value.as_array_mut()) {
                for item in arr.iter_mut() {
                    coerce_json_types(item, items_schema);
                }
            }
        }
        _ => {}
    }
}

/// Coerces string values to boolean.
fn coerce_to_boolean(value: &mut JsonValue, field_name: &str) {
    if let Some(s) = value.as_str() {
        let coerced = match s.to_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        };

        if let Some(b) = coerced {
            debug!(
                field = field_name,
                original = s,
                coerced = b,
                "Coerced string to boolean"
            );
            *value = JsonValue::Bool(b);
        }
    }
}

/// Coerces string values to integer (i64).
fn coerce_to_integer(value: &mut JsonValue, field_name: &str) {
    if let Some(s) = value.as_str() {
        if let Ok(i) = s.parse::<i64>() {
            debug!(
                field = field_name,
                original = s,
                coerced = i,
                "Coerced string to integer"
            );
            *value = JsonValue::Number(i.into());
        }
    }
}

/// Coerces string values to number (f64).
fn coerce_to_number(value: &mut JsonValue, field_name: &str) {
    if let Some(s) = value.as_str() {
        if let Ok(f) = s.parse::<f64>() {
            if let Some(n) = serde_json::Number::from_f64(f) {
                debug!(
                    field = field_name,
                    original = s,
                    coerced = f,
                    "Coerced string to number"
                );
                *value = JsonValue::Number(n);
            }
        }
    }
}

/// Parses tool arguments with lenient type coercion.
///
/// This is a wrapper that first coerces the JSON values to match the schema,
/// then deserializes into the expected type.
///
/// # Arguments
/// * `tool_name` - Name of the tool (for error messages)
/// * `args` - The JSON arguments to parse
/// * `schema` - The JSON Schema describing expected types
///
/// # Returns
/// The deserialized arguments, or a ToolError if parsing fails.
///
/// # Example
/// ```ignore
/// let schema = serde_json::json!({
///     "type": "object",
///     "properties": {
///         "recursive": { "type": "boolean" },
///         "max_depth": { "type": "integer" }
///     }
/// });
///
/// // Input with string "true" instead of boolean true
/// let args = serde_json::json!({"recursive": "true", "max_depth": "2"});
///
/// let parsed: MyArgs = parse_tool_args_lenient("my_tool", args, &schema)?;
/// // parsed.recursive == true
/// // parsed.max_depth == 2
/// ```
pub fn parse_tool_args_lenient<T: DeserializeOwned>(
    tool_name: &str,
    mut args: JsonValue,
    schema: &JsonValue,
) -> Result<T, ToolError> {
    // First, coerce types to match schema
    coerce_json_types(&mut args, schema);

    // Then deserialize
    serde_json::from_value(args.clone()).map_err(|e| {
        ToolError::execution_failed(format!(
            "{}: Invalid arguments: {}. Got: {}",
            tool_name, e, args
        ))
    })
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;

    // =========================================================================
    // IGNORE_PATTERNS Tests
    // =========================================================================

    #[test]
    fn test_ignore_patterns_not_empty() {
        assert!(!IGNORE_PATTERNS.is_empty());
    }

    #[test]
    fn test_ignore_patterns_contains_common_dirs() {
        assert!(IGNORE_PATTERNS.contains(&".git"));
        assert!(IGNORE_PATTERNS.contains(&"node_modules"));
        assert!(IGNORE_PATTERNS.contains(&"target"));
        assert!(IGNORE_PATTERNS.contains(&"__pycache__"));
    }

    // =========================================================================
    // should_ignore Tests
    // =========================================================================

    #[test]
    fn test_should_ignore_git() {
        assert!(should_ignore(".git"));
        assert!(should_ignore(".git/config"));
        assert!(should_ignore("project/.git/HEAD"));
        assert!(should_ignore("/home/user/project/.git"));
    }

    #[test]
    fn test_should_ignore_node_modules() {
        assert!(should_ignore("node_modules"));
        assert!(should_ignore("node_modules/lodash"));
        assert!(should_ignore("project/node_modules/react"));
        assert!(should_ignore("/app/node_modules/package.json"));
    }

    #[test]
    fn test_should_ignore_target() {
        assert!(should_ignore("target"));
        assert!(should_ignore("target/debug"));
        assert!(should_ignore("target/release/binary"));
        assert!(should_ignore("project/target/debug/deps"));
    }

    #[test]
    fn test_should_ignore_python_cache() {
        assert!(should_ignore("__pycache__"));
        assert!(should_ignore("src/__pycache__/module.pyc"));
        assert!(should_ignore(".venv"));
        assert!(should_ignore("venv/lib/python3.9"));
        assert!(should_ignore(".pytest_cache"));
        assert!(should_ignore(".mypy_cache"));
    }

    #[test]
    fn test_should_ignore_ide_dirs() {
        assert!(should_ignore(".idea"));
        assert!(should_ignore(".idea/workspace.xml"));
        assert!(should_ignore(".vscode"));
        assert!(should_ignore(".vscode/settings.json"));
    }

    #[test]
    fn test_should_ignore_build_dirs() {
        assert!(should_ignore("dist"));
        assert!(should_ignore("build"));
        assert!(should_ignore(".next"));
        assert!(should_ignore(".nuxt"));
    }

    #[test]
    fn test_should_ignore_case_insensitive() {
        // Should ignore regardless of case
        assert!(should_ignore("NODE_MODULES"));
        assert!(should_ignore("Node_Modules"));
        assert!(should_ignore(".GIT"));
        assert!(should_ignore("TARGET"));
    }

    #[test]
    fn test_should_not_ignore_normal_paths() {
        assert!(!should_ignore("src"));
        assert!(!should_ignore("lib"));
        assert!(!should_ignore("src/main.rs"));
        assert!(!should_ignore("package.json"));
        assert!(!should_ignore("README.md"));
        assert!(!should_ignore("tests/test_module.py"));
    }

    #[test]
    fn test_should_ignore_substring_matches() {
        // Note: The current implementation uses `contains` which matches substrings.
        // This is by design - it's aggressive about ignoring build artifacts.
        // Words containing patterns like "target" or "build" will be ignored.
        assert!(should_ignore("src/targeting.rs")); // contains "target"
        assert!(should_ignore("src/builder.rs")); // contains "build"
        assert!(should_ignore("rebuild.sh")); // contains "build"

        // These should NOT be ignored (no pattern substring)
        assert!(!should_ignore("src/main.rs"));
        assert!(!should_ignore("lib/utils.py"));
        assert!(!should_ignore("README.md"));
    }

    // =========================================================================
    // get_extension Tests
    // =========================================================================

    #[test]
    fn test_get_extension_common() {
        assert_eq!(get_extension("file.rs"), Some("rs"));
        assert_eq!(get_extension("file.py"), Some("py"));
        assert_eq!(get_extension("file.js"), Some("js"));
        assert_eq!(get_extension("file.json"), Some("json"));
        assert_eq!(get_extension("file.md"), Some("md"));
    }

    #[test]
    fn test_get_extension_with_path() {
        assert_eq!(get_extension("src/main.rs"), Some("rs"));
        assert_eq!(get_extension("/home/user/file.txt"), Some("txt"));
        assert_eq!(get_extension("./relative/path/file.py"), Some("py"));
    }

    #[test]
    fn test_get_extension_multiple_dots() {
        assert_eq!(get_extension("file.test.js"), Some("js"));
        assert_eq!(get_extension("archive.tar.gz"), Some("gz"));
        assert_eq!(get_extension("config.local.json"), Some("json"));
    }

    #[test]
    fn test_get_extension_none() {
        assert_eq!(get_extension("Makefile"), None);
        assert_eq!(get_extension("Dockerfile"), None);
        assert_eq!(get_extension("README"), None);
        assert_eq!(get_extension(".gitignore"), None);
    }

    #[test]
    fn test_get_extension_hidden_file_with_ext() {
        assert_eq!(get_extension(".eslintrc.json"), Some("json"));
        assert_eq!(get_extension(".prettierrc.yaml"), Some("yaml"));
    }

    // =========================================================================
    // is_text_file Tests
    // =========================================================================

    #[test]
    fn test_is_text_file_rust() {
        assert!(is_text_file("main.rs"));
        assert!(is_text_file("lib.rs"));
        assert!(is_text_file("src/module.rs"));
    }

    #[test]
    fn test_is_text_file_python() {
        assert!(is_text_file("script.py"));
        assert!(is_text_file("tests/test_module.py"));
    }

    #[test]
    fn test_is_text_file_javascript() {
        assert!(is_text_file("app.js"));
        assert!(is_text_file("index.ts"));
        assert!(is_text_file("component.tsx"));
        assert!(is_text_file("component.jsx"));
    }

    #[test]
    fn test_is_text_file_config_formats() {
        assert!(is_text_file("config.json"));
        assert!(is_text_file("config.yaml"));
        assert!(is_text_file("config.yml"));
        assert!(is_text_file("Cargo.toml"));
        assert!(is_text_file("settings.ini"));
    }

    #[test]
    fn test_is_text_file_web() {
        assert!(is_text_file("index.html"));
        assert!(is_text_file("styles.css"));
        assert!(is_text_file("styles.scss"));
        assert!(is_text_file("styles.less"));
    }

    #[test]
    fn test_is_text_file_shell() {
        assert!(is_text_file("script.sh"));
        assert!(is_text_file("setup.bash"));
        assert!(is_text_file("config.zsh"));
        assert!(is_text_file("functions.fish"));
    }

    #[test]
    fn test_is_text_file_c_family() {
        assert!(is_text_file("main.c"));
        assert!(is_text_file("header.h"));
        assert!(is_text_file("main.cpp"));
        assert!(is_text_file("header.hpp"));
        assert!(is_text_file("source.cc"));
        assert!(is_text_file("source.cxx"));
    }

    #[test]
    fn test_is_text_file_other_languages() {
        assert!(is_text_file("main.go"));
        assert!(is_text_file("Main.java"));
        assert!(is_text_file("main.kt"));
        assert!(is_text_file("main.swift"));
        assert!(is_text_file("script.rb"));
        assert!(is_text_file("index.php"));
    }

    #[test]
    fn test_is_text_file_data_formats() {
        assert!(is_text_file("query.sql"));
        assert!(is_text_file("schema.graphql"));
        assert!(is_text_file("message.proto"));
        assert!(is_text_file("config.xml"));
        assert!(is_text_file("icon.svg"));
    }

    #[test]
    fn test_is_text_file_special_files() {
        assert!(is_text_file("Makefile"));
        assert!(is_text_file("Dockerfile"));
        assert!(is_text_file("Rakefile"));
        assert!(is_text_file("Gemfile"));
        assert!(is_text_file(".gitignore"));
        assert!(is_text_file(".env"));
    }

    #[test]
    fn test_is_text_file_case_insensitive_extension() {
        assert!(is_text_file("FILE.RS"));
        assert!(is_text_file("FILE.Py"));
        assert!(is_text_file("FILE.JSON"));
    }

    #[test]
    fn test_is_not_text_file_binary() {
        assert!(!is_text_file("image.png"));
        assert!(!is_text_file("image.jpg"));
        assert!(!is_text_file("image.gif"));
        assert!(!is_text_file("document.pdf"));
        assert!(!is_text_file("archive.zip"));
        assert!(!is_text_file("binary.exe"));
        assert!(!is_text_file("library.so"));
        assert!(!is_text_file("library.dll"));
    }

    #[test]
    fn test_is_not_text_file_unknown() {
        assert!(!is_text_file("file.unknown"));
        assert!(!is_text_file("random.xyz"));
    }

    // =========================================================================
    // coerce_json_types Tests - Boolean Coercion
    // =========================================================================

    #[test]
    fn test_coerce_boolean_from_string_true() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" }
            }
        });

        // Test lowercase "true"
        let mut args = serde_json::json!({ "flag": "true" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(true));

        // Test uppercase "TRUE"
        let mut args = serde_json::json!({ "flag": "TRUE" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(true));

        // Test mixed case "True"
        let mut args = serde_json::json!({ "flag": "True" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(true));

        // Test "1"
        let mut args = serde_json::json!({ "flag": "1" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(true));
    }

    #[test]
    fn test_coerce_boolean_from_string_false() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" }
            }
        });

        // Test lowercase "false"
        let mut args = serde_json::json!({ "flag": "false" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(false));

        // Test uppercase "FALSE"
        let mut args = serde_json::json!({ "flag": "FALSE" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(false));

        // Test mixed case "False"
        let mut args = serde_json::json!({ "flag": "False" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(false));

        // Test "0"
        let mut args = serde_json::json!({ "flag": "0" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(false));
    }

    #[test]
    fn test_coerce_boolean_already_correct() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" }
            }
        });

        // Already a boolean true
        let mut args = serde_json::json!({ "flag": true });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(true));

        // Already a boolean false
        let mut args = serde_json::json!({ "flag": false });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(false));
    }

    #[test]
    fn test_coerce_boolean_invalid_string_unchanged() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" }
            }
        });

        // Invalid string should remain unchanged
        let mut args = serde_json::json!({ "flag": "yes" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!("yes"));

        let mut args = serde_json::json!({ "flag": "no" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!("no"));

        let mut args = serde_json::json!({ "flag": "garbage" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!("garbage"));
    }

    // =========================================================================
    // coerce_json_types Tests - Integer Coercion
    // =========================================================================

    #[test]
    fn test_coerce_integer_from_string() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer" }
            }
        });

        let mut args = serde_json::json!({ "count": "42" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["count"], serde_json::json!(42));

        // Negative integer
        let mut args = serde_json::json!({ "count": "-123" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["count"], serde_json::json!(-123));

        // Zero
        let mut args = serde_json::json!({ "count": "0" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["count"], serde_json::json!(0));
    }

    #[test]
    fn test_coerce_integer_already_correct() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer" }
            }
        });

        let mut args = serde_json::json!({ "count": 42 });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["count"], serde_json::json!(42));
    }

    #[test]
    fn test_coerce_integer_invalid_string_unchanged() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "count": { "type": "integer" }
            }
        });

        // Float string should NOT be coerced to integer
        let mut args = serde_json::json!({ "count": "3.14" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["count"], serde_json::json!("3.14"));

        // Non-numeric string unchanged
        let mut args = serde_json::json!({ "count": "hello" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["count"], serde_json::json!("hello"));
    }

    // =========================================================================
    // coerce_json_types Tests - Number (float) Coercion
    // =========================================================================

    #[test]
    fn test_coerce_number_from_string() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "value": { "type": "number" }
            }
        });

        let mut args = serde_json::json!({ "value": "3.14" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["value"], serde_json::json!(3.14));

        // Integer string should also work for number type
        let mut args = serde_json::json!({ "value": "42" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["value"], serde_json::json!(42.0));

        // Negative float
        let mut args = serde_json::json!({ "value": "-2.718" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["value"], serde_json::json!(-2.718));

        // Scientific notation
        let mut args = serde_json::json!({ "value": "1.5e10" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["value"], serde_json::json!(1.5e10));
    }

    #[test]
    fn test_coerce_number_already_correct() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "value": { "type": "number" }
            }
        });

        let mut args = serde_json::json!({ "value": 3.14 });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["value"], serde_json::json!(3.14));
    }

    #[test]
    fn test_coerce_number_invalid_string_unchanged() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "value": { "type": "number" }
            }
        });

        let mut args = serde_json::json!({ "value": "not_a_number" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["value"], serde_json::json!("not_a_number"));
    }

    // =========================================================================
    // coerce_json_types Tests - Multiple Fields
    // =========================================================================

    #[test]
    fn test_coerce_multiple_fields() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "recursive": { "type": "boolean" },
                "max_depth": { "type": "integer" },
                "threshold": { "type": "number" }
            }
        });

        let mut args = serde_json::json!({
            "recursive": "true",
            "max_depth": "2",
            "threshold": "0.5"
        });
        coerce_json_types(&mut args, &schema);

        assert_eq!(args["recursive"], serde_json::json!(true));
        assert_eq!(args["max_depth"], serde_json::json!(2));
        assert_eq!(args["threshold"], serde_json::json!(0.5));
    }

    #[test]
    fn test_coerce_missing_fields_unchanged() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" },
                "count": { "type": "integer" }
            }
        });

        // Only flag is present
        let mut args = serde_json::json!({ "flag": "true" });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(true));
        assert!(args.get("count").is_none());
    }

    #[test]
    fn test_coerce_extra_fields_unchanged() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" }
            }
        });

        // Extra field "name" not in schema
        let mut args = serde_json::json!({
            "flag": "true",
            "name": "test"
        });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(true));
        assert_eq!(args["name"], serde_json::json!("test"));
    }

    // =========================================================================
    // coerce_json_types Tests - Nested Objects
    // =========================================================================

    #[test]
    fn test_coerce_nested_object() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "config": {
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean" },
                        "limit": { "type": "integer" }
                    }
                }
            }
        });

        let mut args = serde_json::json!({
            "config": {
                "enabled": "true",
                "limit": "100"
            }
        });
        coerce_json_types(&mut args, &schema);

        assert_eq!(args["config"]["enabled"], serde_json::json!(true));
        assert_eq!(args["config"]["limit"], serde_json::json!(100));
    }

    // =========================================================================
    // coerce_json_types Tests - Arrays
    // =========================================================================

    #[test]
    fn test_coerce_array_items() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "active": { "type": "boolean" },
                            "count": { "type": "integer" }
                        }
                    }
                }
            }
        });

        let mut args = serde_json::json!({
            "items": [
                { "active": "true", "count": "1" },
                { "active": "false", "count": "2" }
            ]
        });
        coerce_json_types(&mut args, &schema);

        assert_eq!(args["items"][0]["active"], serde_json::json!(true));
        assert_eq!(args["items"][0]["count"], serde_json::json!(1));
        assert_eq!(args["items"][1]["active"], serde_json::json!(false));
        assert_eq!(args["items"][1]["count"], serde_json::json!(2));
    }

    // =========================================================================
    // coerce_json_types Tests - Edge Cases
    // =========================================================================

    #[test]
    fn test_coerce_empty_object() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" }
            }
        });

        let mut args = serde_json::json!({});
        coerce_json_types(&mut args, &schema);
        assert_eq!(args, serde_json::json!({}));
    }

    #[test]
    fn test_coerce_null_value_unchanged() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" }
            }
        });

        let mut args = serde_json::json!({ "flag": null });
        coerce_json_types(&mut args, &schema);
        assert_eq!(args["flag"], serde_json::json!(null));
    }

    #[test]
    fn test_coerce_schema_without_properties() {
        // Direct type schema (not wrapped in properties)
        let schema = serde_json::json!({
            "type": "boolean"
        });

        let mut args = serde_json::json!("true");
        coerce_json_types(&mut args, &schema);
        assert_eq!(args, serde_json::json!(true));
    }

    #[test]
    fn test_coerce_schema_without_type() {
        // Schema with no type should not crash
        let schema = serde_json::json!({
            "description": "No type here"
        });

        let mut args = serde_json::json!({ "flag": "true" });
        coerce_json_types(&mut args, &schema);
        // Should remain unchanged
        assert_eq!(args["flag"], serde_json::json!("true"));
    }

    // =========================================================================
    // parse_tool_args_lenient Tests
    // =========================================================================

    #[test]
    fn test_parse_tool_args_lenient_success() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct MyArgs {
            recursive: bool,
            max_depth: i32,
        }

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "recursive": { "type": "boolean" },
                "max_depth": { "type": "integer" }
            }
        });

        let args = serde_json::json!({
            "recursive": "true",
            "max_depth": "2"
        });

        let result: Result<MyArgs, _> = parse_tool_args_lenient("test_tool", args, &schema);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert!(parsed.recursive);
        assert_eq!(parsed.max_depth, 2);
    }

    #[test]
    fn test_parse_tool_args_lenient_already_correct_types() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct MyArgs {
            flag: bool,
            count: i64,
        }

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "flag": { "type": "boolean" },
                "count": { "type": "integer" }
            }
        });

        let args = serde_json::json!({
            "flag": true,
            "count": 42
        });

        let result: Result<MyArgs, _> = parse_tool_args_lenient("test_tool", args, &schema);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert!(parsed.flag);
        assert_eq!(parsed.count, 42);
    }

    #[test]
    fn test_parse_tool_args_lenient_error() {
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct MyArgs {
            required_field: String,
        }

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "required_field": { "type": "string" }
            }
        });

        // Missing required field
        let args = serde_json::json!({});

        let result: Result<MyArgs, _> = parse_tool_args_lenient("test_tool", args, &schema);
        assert!(result.is_err());

        let err = result.unwrap_err();
        let err_string = format!("{:?}", err);
        assert!(err_string.contains("test_tool"));
    }

    #[test]
    fn test_parse_tool_args_lenient_mixed_types() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct MyArgs {
            name: String,
            enabled: bool,
            count: i32,
            ratio: f64,
        }

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "enabled": { "type": "boolean" },
                "count": { "type": "integer" },
                "ratio": { "type": "number" }
            }
        });

        let args = serde_json::json!({
            "name": "test",
            "enabled": "TRUE",
            "count": "5",
            "ratio": "0.75"
        });

        let result: Result<MyArgs, _> = parse_tool_args_lenient("test_tool", args, &schema);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert_eq!(parsed.name, "test");
        assert!(parsed.enabled);
        assert_eq!(parsed.count, 5);
        assert!((parsed.ratio - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_tool_args_lenient_with_optional() {
        #[derive(Debug, serde::Deserialize, PartialEq)]
        struct MyArgs {
            required: bool,
            #[serde(default)]
            optional: Option<i32>,
        }

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "required": { "type": "boolean" },
                "optional": { "type": "integer" }
            }
        });

        // With optional field
        let args = serde_json::json!({
            "required": "true",
            "optional": "10"
        });

        let result: Result<MyArgs, _> = parse_tool_args_lenient("test_tool", args, &schema);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert!(parsed.required);
        assert_eq!(parsed.optional, Some(10));

        // Without optional field
        let args = serde_json::json!({
            "required": "false"
        });

        let result: Result<MyArgs, _> = parse_tool_args_lenient("test_tool", args, &schema);
        assert!(result.is_ok());

        let parsed = result.unwrap();
        assert!(!parsed.required);
        assert_eq!(parsed.optional, None);
    }
}
