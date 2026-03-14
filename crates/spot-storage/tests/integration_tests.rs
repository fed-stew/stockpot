//! Integration tests for spot-storage public API.
//!
//! Tests cover:
//! - Database creation, migration, and CRUD operations
//! - PoolKey operations
//! - Settings repository
//! - Concurrent access patterns
//! - Cross-table isolation

use spot_storage::Database;
use tempfile::TempDir;

fn setup() -> (TempDir, Database) {
    let tmp = TempDir::new().unwrap();
    let db = Database::open_at(tmp.path().join("test.db")).unwrap();
    db.migrate().unwrap();
    (tmp, db)
}

// =========================================================================
// Database Lifecycle Tests
// =========================================================================

#[test]
fn test_database_open_migrate_reopen() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("lifecycle.db");

    // First open: create and populate
    {
        let db = Database::open_at(db_path.clone()).unwrap();
        db.migrate().unwrap();
        db.save_api_key("KEY_A", "value_a").unwrap();
        db.save_pool_key("PROVIDER", "pool_key_1", Some("label"), None)
            .unwrap();
    }

    // Second open: verify data persists without re-migration
    {
        let db = Database::open_at(db_path.clone()).unwrap();
        assert_eq!(
            db.get_api_key("KEY_A").unwrap(),
            Some("value_a".to_string())
        );
        let pool_keys = db.get_pool_keys("PROVIDER").unwrap();
        assert_eq!(pool_keys.len(), 1);
        assert_eq!(pool_keys[0].api_key, "pool_key_1");
    }

    // Third open: re-migrate (idempotent)
    {
        let db = Database::open_at(db_path).unwrap();
        db.migrate().unwrap();
        // Data should still be there
        assert!(db.has_api_key("KEY_A"));
        assert!(db.has_pool_keys("PROVIDER"));
    }
}

// =========================================================================
// Cross-Repository Isolation Tests
// =========================================================================

#[test]
fn test_api_keys_and_pool_keys_are_independent() {
    let (_tmp, db) = setup();

    // Save same key name in both tables
    db.save_api_key("OPENAI_API_KEY", "legacy_key").unwrap();
    db.save_pool_key("OPENAI_API_KEY", "pool_key_1", None, None)
        .unwrap();

    // Deleting from api_keys shouldn't affect pool
    db.delete_api_key("OPENAI_API_KEY").unwrap();
    assert!(!db.has_api_key("OPENAI_API_KEY"));
    assert!(db.has_pool_keys("OPENAI_API_KEY"));

    // And vice versa
    db.save_api_key("OPENAI_API_KEY", "new_legacy").unwrap();
    let pool_keys = db.get_pool_keys("OPENAI_API_KEY").unwrap();
    let id = pool_keys[0].id;
    db.delete_pool_key(id).unwrap();

    assert!(db.has_api_key("OPENAI_API_KEY"));
    assert!(!db.has_pool_keys("OPENAI_API_KEY"));
}

// =========================================================================
// Settings Repository Tests
// =========================================================================

#[test]
fn test_settings_crud() {
    let (_tmp, db) = setup();
    let settings = db.settings_repo();

    // Initially empty
    assert!(settings.get("theme").unwrap().is_none());

    // Set a value
    settings.set("theme", "dark").unwrap();
    assert_eq!(settings.get("theme").unwrap(), Some("dark".to_string()));

    // Update the value
    settings.set("theme", "light").unwrap();
    assert_eq!(settings.get("theme").unwrap(), Some("light".to_string()));

    // Delete the value
    settings.delete("theme").unwrap();
    assert!(settings.get("theme").unwrap().is_none());
}

#[test]
fn test_settings_list() {
    let (_tmp, db) = setup();
    let settings = db.settings_repo();

    settings.set("b_key", "b_value").unwrap();
    settings.set("a_key", "a_value").unwrap();
    settings.set("c_key", "c_value").unwrap();

    let all = settings.list().unwrap();
    // Should be sorted by key
    assert!(all.len() >= 3);
    let keys: Vec<&str> = all.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn test_settings_list_with_prefix() {
    let (_tmp, db) = setup();
    let settings = db.settings_repo();

    settings.set("model.gpt4.temperature", "0.7").unwrap();
    settings.set("model.gpt4.max_tokens", "4096").unwrap();
    settings.set("model.claude.temperature", "0.5").unwrap();
    settings.set("ui.theme", "dark").unwrap();

    let model_settings = settings.list_with_prefix("model.").unwrap();
    assert_eq!(model_settings.len(), 3);

    let gpt4_settings = settings.list_with_prefix("model.gpt4.").unwrap();
    assert_eq!(gpt4_settings.len(), 2);

    let ui_settings = settings.list_with_prefix("ui.").unwrap();
    assert_eq!(ui_settings.len(), 1);
    assert_eq!(ui_settings[0], ("ui.theme".to_string(), "dark".to_string()));
}

#[test]
fn test_settings_delete_nonexistent() {
    let (_tmp, db) = setup();
    let settings = db.settings_repo();

    // Should not error
    settings.delete("nonexistent_key").unwrap();
}

#[test]
fn test_settings_special_characters() {
    let (_tmp, db) = setup();
    let settings = db.settings_repo();

    let key = "model_settings.custom/model.temperature";
    let value = r#"{"nested": "json with 'quotes' and \"escapes\""}"#;

    settings.set(key, value).unwrap();
    assert_eq!(settings.get(key).unwrap(), Some(value.to_string()));
}

// =========================================================================
// PoolKey Rotation Pattern Tests
// =========================================================================

#[test]
fn test_pool_key_rotation_workflow() {
    let (_tmp, db) = setup();

    // Add 3 keys with ascending priority
    let key1 = db
        .save_pool_key("OPENAI_API_KEY", "sk-key-1", Some("Primary"), Some(0))
        .unwrap();
    let key2 = db
        .save_pool_key("OPENAI_API_KEY", "sk-key-2", Some("Secondary"), Some(1))
        .unwrap();
    let key3 = db
        .save_pool_key("OPENAI_API_KEY", "sk-key-3", Some("Tertiary"), Some(2))
        .unwrap();

    // All active initially
    assert_eq!(db.count_active_pool_keys("OPENAI_API_KEY").unwrap(), 3);

    // Mark key1 as used
    db.mark_key_used(key1).unwrap();
    let keys = db.get_active_pool_keys("OPENAI_API_KEY").unwrap();
    assert_eq!(keys[0].id, key1); // Still first by priority
    assert!(keys[0].last_used_at.is_some());

    // Simulate rate limit on key1: mark errors
    db.mark_key_error(key1).unwrap();
    db.mark_key_error(key1).unwrap();
    db.mark_key_error(key1).unwrap();

    let keys = db.get_pool_keys("OPENAI_API_KEY").unwrap();
    let k1 = keys.iter().find(|k| k.id == key1).unwrap();
    assert_eq!(k1.error_count, 3);

    // Deactivate key1 after too many errors
    db.set_key_active(key1, false).unwrap();
    assert_eq!(db.count_active_pool_keys("OPENAI_API_KEY").unwrap(), 2);

    // key2 should now be first active
    let active = db.get_active_pool_keys("OPENAI_API_KEY").unwrap();
    assert_eq!(active[0].id, key2);

    // Later: reactivate key1 and reset errors
    db.set_key_active(key1, true).unwrap();
    db.reset_key_errors(key1).unwrap();

    let restored = db.get_pool_keys("OPENAI_API_KEY").unwrap();
    let k1_restored = restored.iter().find(|k| k.id == key1).unwrap();
    assert_eq!(k1_restored.error_count, 0);
    assert!(k1_restored.is_active);

    // Reprioritize: move key3 to highest priority
    db.update_key_priority(key3, 0).unwrap();
    db.update_key_priority(key1, 2).unwrap();

    let reordered = db.get_active_pool_keys("OPENAI_API_KEY").unwrap();
    assert_eq!(reordered[0].id, key3);

    // Clean up: delete all keys
    db.delete_pool_key(key1).unwrap();
    db.delete_pool_key(key2).unwrap();
    db.delete_pool_key(key3).unwrap();
    assert!(!db.has_pool_keys("OPENAI_API_KEY"));
}

// =========================================================================
// Concurrent Access Tests
// =========================================================================

#[test]
fn test_concurrent_reads_from_same_database() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("concurrent.db");

    // Setup data
    let db = Database::open_at(db_path.clone()).unwrap();
    db.migrate().unwrap();
    db.save_api_key("SHARED_KEY", "shared_value").unwrap();
    drop(db);

    // Open two separate connections and read concurrently
    let db1 = Database::open_at(db_path.clone()).unwrap();
    let db2 = Database::open_at(db_path).unwrap();

    let val1 = db1.get_api_key("SHARED_KEY").unwrap();
    let val2 = db2.get_api_key("SHARED_KEY").unwrap();

    assert_eq!(val1, Some("shared_value".to_string()));
    assert_eq!(val2, Some("shared_value".to_string()));
}

#[test]
fn test_write_then_read_from_different_connections() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("write_read.db");

    // Setup
    let db_setup = Database::open_at(db_path.clone()).unwrap();
    db_setup.migrate().unwrap();
    drop(db_setup);

    // Write from connection 1
    let db_writer = Database::open_at(db_path.clone()).unwrap();
    db_writer
        .save_api_key("WRITTEN_KEY", "from_writer")
        .unwrap();
    drop(db_writer);

    // Read from connection 2
    let db_reader = Database::open_at(db_path).unwrap();
    let value = db_reader.get_api_key("WRITTEN_KEY").unwrap();
    assert_eq!(value, Some("from_writer".to_string()));
}

// =========================================================================
// Edge Cases
// =========================================================================

#[test]
fn test_empty_provider_name_pool_key() {
    let (_tmp, db) = setup();

    let id = db
        .save_pool_key("", "key_for_empty_provider", None, None)
        .unwrap();
    assert!(id > 0);

    let keys = db.get_pool_keys("").unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].api_key, "key_for_empty_provider");
}

#[test]
fn test_large_number_of_pool_keys() {
    let (_tmp, db) = setup();

    // Insert 50 keys for the same provider
    for i in 0..50 {
        db.save_pool_key(
            "BULK_PROVIDER",
            &format!("key-{}", i),
            Some(&format!("Key #{}", i)),
            Some(i),
        )
        .unwrap();
    }

    let keys = db.get_pool_keys("BULK_PROVIDER").unwrap();
    assert_eq!(keys.len(), 50);

    let active = db.get_active_pool_keys("BULK_PROVIDER").unwrap();
    assert_eq!(active.len(), 50);

    // Verify ordering by priority
    for i in 0..49 {
        assert!(active[i].priority <= active[i + 1].priority);
    }
}

#[test]
fn test_settings_repo_accessor() {
    let (_tmp, db) = setup();

    // Test that we can use the settings repo accessor multiple times
    db.settings_repo().set("key1", "val1").unwrap();
    db.settings_repo().set("key2", "val2").unwrap();

    let val1 = db.settings_repo().get("key1").unwrap();
    let val2 = db.settings_repo().get("key2").unwrap();

    assert_eq!(val1, Some("val1".to_string()));
    assert_eq!(val2, Some("val2".to_string()));
}
