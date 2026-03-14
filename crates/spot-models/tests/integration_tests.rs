//! Integration tests for spot-models public API.
//!
//! Tests cover:
//! - ModelRegistry operations (add, get, list, load from DB)
//! - ModelConfig serialization/deserialization roundtrip
//! - ModelType coverage
//! - Catalog loading patterns

use spot_models::{ModelConfig, ModelRegistry, ModelType};
use spot_storage::Database;
use std::collections::HashMap;
use tempfile::TempDir;

fn setup() -> (TempDir, Database) {
    let tmp = TempDir::new().unwrap();
    let db = Database::open_at(tmp.path().join("test.db")).unwrap();
    db.migrate().unwrap();
    (tmp, db)
}

// =========================================================================
// ModelConfig Serialization Roundtrip Tests
// =========================================================================

#[test]
fn test_model_config_json_roundtrip_minimal() {
    let config = ModelConfig {
        name: "test-model".to_string(),
        model_type: ModelType::Openai,
        context_length: 4096,
        ..Default::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: ModelConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.name, "test-model");
    assert_eq!(parsed.model_type, ModelType::Openai);
    assert_eq!(parsed.context_length, 4096);
}

#[test]
fn test_model_config_json_roundtrip_full() {
    let config = ModelConfig {
        name: "full-model".to_string(),
        model_type: ModelType::CustomOpenai,
        model_id: Some("custom-id-v1".to_string()),
        context_length: 200_000,
        supports_thinking: true,
        supports_vision: true,
        supports_tools: false,
        description: Some("A fully configured model".to_string()),
        custom_endpoint: Some(spot_models::CustomEndpoint {
            url: "https://api.example.com/v1".to_string(),
            api_key: Some("$MY_API_KEY".to_string()),
            headers: {
                let mut h = HashMap::new();
                h.insert("X-Custom".to_string(), "value".to_string());
                h
            },
            ca_certs_path: Some("/etc/ssl/certs/ca.pem".to_string()),
        }),
        azure_deployment: Some("my-deployment".to_string()),
        azure_api_version: Some("2024-01-01".to_string()),
        round_robin_models: vec!["model-a".to_string(), "model-b".to_string()],
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: ModelConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.name, config.name);
    assert_eq!(parsed.model_type, config.model_type);
    assert_eq!(parsed.model_id, config.model_id);
    assert_eq!(parsed.context_length, config.context_length);
    assert_eq!(parsed.supports_thinking, config.supports_thinking);
    assert_eq!(parsed.supports_vision, config.supports_vision);
    assert_eq!(parsed.supports_tools, config.supports_tools);
    assert_eq!(parsed.description, config.description);
    assert!(parsed.custom_endpoint.is_some());
    let ep = parsed.custom_endpoint.as_ref().unwrap();
    assert_eq!(ep.url, "https://api.example.com/v1");
    assert_eq!(ep.api_key, Some("$MY_API_KEY".to_string()));
    assert_eq!(ep.headers.get("X-Custom"), Some(&"value".to_string()));
    assert_eq!(parsed.azure_deployment, config.azure_deployment);
    assert_eq!(parsed.azure_api_version, config.azure_api_version);
    assert_eq!(parsed.round_robin_models, config.round_robin_models);
}

// =========================================================================
// ModelConfig Helper Methods
// =========================================================================

#[test]
fn test_effective_model_id() {
    let config_with_id = ModelConfig {
        name: "display-name".to_string(),
        model_id: Some("actual-api-id".to_string()),
        ..Default::default()
    };
    assert_eq!(config_with_id.effective_model_id(), "actual-api-id");

    let config_without_id = ModelConfig {
        name: "falls-back-to-name".to_string(),
        model_id: None,
        ..Default::default()
    };
    assert_eq!(config_without_id.effective_model_id(), "falls-back-to-name");
}

#[test]
fn test_is_oauth() {
    let oauth_types = [ModelType::ClaudeCode, ModelType::ChatgptOauth];
    let non_oauth_types = [
        ModelType::Openai,
        ModelType::Anthropic,
        ModelType::Gemini,
        ModelType::CustomOpenai,
        ModelType::CustomAnthropic,
        ModelType::AzureOpenai,
        ModelType::GoogleVertex,
        ModelType::Openrouter,
        ModelType::RoundRobin,
    ];

    for mt in oauth_types {
        let config = ModelConfig {
            model_type: mt,
            ..Default::default()
        };
        assert!(config.is_oauth(), "{:?} should be OAuth", mt);
    }

    for mt in non_oauth_types {
        let config = ModelConfig {
            model_type: mt,
            ..Default::default()
        };
        assert!(!config.is_oauth(), "{:?} should not be OAuth", mt);
    }
}

#[test]
fn test_requires_custom_endpoint() {
    let custom_types = [ModelType::CustomOpenai, ModelType::CustomAnthropic];
    let non_custom_types = [
        ModelType::Openai,
        ModelType::Anthropic,
        ModelType::ClaudeCode,
    ];

    for mt in custom_types {
        let config = ModelConfig {
            model_type: mt,
            ..Default::default()
        };
        assert!(
            config.requires_custom_endpoint(),
            "{:?} should require custom endpoint",
            mt
        );
    }

    for mt in non_custom_types {
        let config = ModelConfig {
            model_type: mt,
            ..Default::default()
        };
        assert!(
            !config.requires_custom_endpoint(),
            "{:?} should not require custom endpoint",
            mt
        );
    }
}

// =========================================================================
// ModelType Display
// =========================================================================

#[test]
fn test_all_model_types_have_display() {
    let types = [
        ModelType::Openai,
        ModelType::Anthropic,
        ModelType::Gemini,
        ModelType::CustomOpenai,
        ModelType::CustomAnthropic,
        ModelType::ClaudeCode,
        ModelType::ChatgptOauth,
        ModelType::AzureOpenai,
        ModelType::GoogleVertex,
        ModelType::Openrouter,
        ModelType::RoundRobin,
    ];

    for mt in types {
        let display = mt.to_string();
        assert!(!display.is_empty(), "{:?} has empty display", mt);
    }
}

// =========================================================================
// ModelRegistry In-Memory Operations
// =========================================================================

#[test]
fn test_registry_add_get_contains_cycle() {
    let mut registry = ModelRegistry::new();
    assert!(registry.is_empty());

    let config = ModelConfig {
        name: "my-model".to_string(),
        model_type: ModelType::Anthropic,
        context_length: 200_000,
        supports_thinking: true,
        ..Default::default()
    };

    registry.add(config);

    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 1);
    assert!(registry.contains("my-model"));
    assert!(!registry.contains("other-model"));

    let retrieved = registry.get("my-model").unwrap();
    assert_eq!(retrieved.model_type, ModelType::Anthropic);
    assert_eq!(retrieved.context_length, 200_000);
    assert!(retrieved.supports_thinking);
}

#[test]
fn test_registry_list_sorted() {
    let mut registry = ModelRegistry::new();
    registry.add(ModelConfig {
        name: "zebra".to_string(),
        ..Default::default()
    });
    registry.add(ModelConfig {
        name: "alpha".to_string(),
        ..Default::default()
    });
    registry.add(ModelConfig {
        name: "middle".to_string(),
        ..Default::default()
    });

    let names = registry.list();
    assert_eq!(names, vec!["alpha", "middle", "zebra"]);
}

// =========================================================================
// ModelRegistry Database Operations
// =========================================================================

#[test]
fn test_registry_db_roundtrip() {
    let (_tmp, db) = setup();

    // Add models to DB
    let configs = vec![
        ModelConfig {
            name: "gpt-4o".to_string(),
            model_type: ModelType::Openai,
            model_id: Some("gpt-4o-2024-08-06".to_string()),
            context_length: 128_000,
            supports_vision: true,
            supports_tools: true,
            description: Some("GPT-4o flagship".to_string()),
            ..Default::default()
        },
        ModelConfig {
            name: "claude-3-opus".to_string(),
            model_type: ModelType::Anthropic,
            context_length: 200_000,
            supports_thinking: true,
            supports_vision: true,
            ..Default::default()
        },
    ];

    for config in &configs {
        ModelRegistry::add_model_to_db(&db, config).unwrap();
    }

    // Load from DB
    let registry = ModelRegistry::load_from_db(&db).unwrap();
    assert_eq!(registry.len(), 2);

    let gpt = registry.get("gpt-4o").unwrap();
    assert_eq!(gpt.model_id, Some("gpt-4o-2024-08-06".to_string()));
    assert_eq!(gpt.context_length, 128_000);

    let claude = registry.get("claude-3-opus").unwrap();
    assert!(claude.supports_thinking);
    assert_eq!(claude.context_length, 200_000);
}

#[test]
fn test_registry_remove_and_reload() {
    let (_tmp, db) = setup();

    let config = ModelConfig {
        name: "to-remove".to_string(),
        ..Default::default()
    };

    ModelRegistry::add_model_to_db(&db, &config).unwrap();

    // Verify it exists
    let reg = ModelRegistry::load_from_db(&db).unwrap();
    assert!(reg.contains("to-remove"));

    // Remove
    ModelRegistry::remove_model_from_db(&db, "to-remove").unwrap();

    // Verify it's gone after reload
    let reg = ModelRegistry::load_from_db(&db).unwrap();
    assert!(!reg.contains("to-remove"));
}

#[test]
fn test_registry_reload_replaces_in_memory() {
    let (_tmp, db) = setup();

    let mut registry = ModelRegistry::new();

    // Add in-memory only model
    registry.add(ModelConfig {
        name: "memory-only".to_string(),
        ..Default::default()
    });

    // Add model to DB
    ModelRegistry::add_model_to_db(
        &db,
        &ModelConfig {
            name: "db-only".to_string(),
            ..Default::default()
        },
    )
    .unwrap();

    // Reload from DB should replace in-memory
    registry.reload_from_db(&db).unwrap();

    assert!(!registry.contains("memory-only"));
    assert!(registry.contains("db-only"));
}

// =========================================================================
// ModelRegistry File Loading
// =========================================================================

#[test]
fn test_registry_load_file() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("models.json");

    let json = r#"[
        {
            "name": "file-model-1",
            "model_type": "openai",
            "context_length": 4096
        },
        {
            "name": "file-model-2",
            "model_type": "anthropic",
            "context_length": 200000,
            "supports_thinking": true
        }
    ]"#;
    std::fs::write(&file, json).unwrap();

    let mut registry = ModelRegistry::new();
    registry.load_file(&file).unwrap();

    assert_eq!(registry.len(), 2);
    let m1 = registry.get("file-model-1").unwrap();
    assert_eq!(m1.model_type, ModelType::Openai);
    assert_eq!(m1.context_length, 4096);

    let m2 = registry.get("file-model-2").unwrap();
    assert_eq!(m2.model_type, ModelType::Anthropic);
    assert!(m2.supports_thinking);
}

#[test]
fn test_registry_load_file_error_handling() {
    let mut registry = ModelRegistry::new();

    // Non-existent file
    let result = registry.load_file(&std::path::PathBuf::from("/nonexistent/models.json"));
    assert!(result.is_err());

    // Invalid JSON
    let dir = TempDir::new().unwrap();
    let bad_file = dir.path().join("bad.json");
    std::fs::write(&bad_file, "not json").unwrap();
    let result = registry.load_file(&bad_file);
    assert!(result.is_err());
}

// =========================================================================
// Provider Availability Tests
// =========================================================================

#[test]
fn test_list_available_requires_api_key() {
    let (_tmp, db) = setup();

    // Add OpenAI model without API key
    ModelRegistry::add_model_to_db(
        &db,
        &ModelConfig {
            name: "no-key-model".to_string(),
            model_type: ModelType::Openai,
            ..Default::default()
        },
    )
    .unwrap();

    let registry = ModelRegistry::load_from_db(&db).unwrap();
    let available = registry.list_available(&db);
    assert!(
        !available.contains(&"no-key-model".to_string()),
        "Model without API key should not be available"
    );

    // Now add the API key
    db.save_api_key("OPENAI_API_KEY", "sk-test").unwrap();
    let available = registry.list_available(&db);
    assert!(
        available.contains(&"no-key-model".to_string()),
        "Model with API key should be available"
    );
}

#[test]
fn test_list_available_pool_keys_work() {
    let (_tmp, db) = setup();

    ModelRegistry::add_model_to_db(
        &db,
        &ModelConfig {
            name: "pool-model".to_string(),
            model_type: ModelType::Anthropic,
            ..Default::default()
        },
    )
    .unwrap();

    // No keys at all - not available
    let registry = ModelRegistry::load_from_db(&db).unwrap();
    assert!(!registry
        .list_available(&db)
        .contains(&"pool-model".to_string()));

    // Add pool key
    db.save_pool_key("ANTHROPIC_API_KEY", "sk-ant-pool", None, None)
        .unwrap();

    assert!(registry
        .list_available(&db)
        .contains(&"pool-model".to_string()));
}
