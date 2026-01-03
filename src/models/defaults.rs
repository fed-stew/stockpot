//! Featured model recommendations.
//!
//! This module provides a curated list of "featured" models to highlight
//! in the UI. These are NOT auto-configured - users must add them via
//! `/add_model` or have the appropriate API keys set up.
//!
//! The full model catalog is downloaded from models.dev at build time.
//! See `build.rs` for details.

/// Featured models to highlight in the UI.
///
/// Format: "provider:model_id"
/// These are popular, well-tested models that we recommend.
pub fn featured_models() -> &'static [&'static str] {
    &[
        // OpenAI
        "openai:gpt-4o",
        "openai:gpt-4o-mini",
        "openai:o1",
        // Anthropic
        "anthropic:claude-sonnet-4-20250514",
        "anthropic:claude-opus-4-20250514",
        // Google
        "gemini:gemini-2.0-flash",
        "gemini:gemini-2.5-pro",
    ]
}

/// Check if a model is featured.
pub fn is_featured(model_name: &str) -> bool {
    // Handle both prefixed and unprefixed names
    featured_models()
        .iter()
        .any(|&featured| featured == model_name || featured.ends_with(&format!(":{}", model_name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_featured_models_not_empty() {
        assert!(!featured_models().is_empty());
    }

    #[test]
    fn test_featured_models_have_prefix() {
        for model in featured_models() {
            assert!(
                model.contains(':'),
                "Featured model should have provider prefix: {}",
                model
            );
        }
    }

    #[test]
    fn test_is_featured() {
        assert!(is_featured("openai:gpt-4o"));
        assert!(is_featured("gpt-4o")); // Unprefixed also works
        assert!(!is_featured("some-random-model"));
    }
}
