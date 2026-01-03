//! Build script for stockpot - downloads models catalog at build time.
//!
//! This script fetches the model catalog from https://models.dev/api.json
//! and embeds it into the binary. It includes:
//! - 24-hour cache to avoid unnecessary downloads
//! - Fallback to cached version if network fails
//! - Force refresh via FORCE_CATALOG_REFRESH=1 env var
//! - 10-second timeout for network requests

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const CATALOG_URL: &str = "https://models.dev/api.json";
const CATALOG_FILENAME: &str = "models_catalog.json";
const CACHE_MAX_AGE_SECS: u64 = 24 * 60 * 60; // 24 hours
const REQUEST_TIMEOUT_SECS: u64 = 10;

fn main() {
    // Tell Cargo to rerun if these change
    println!("cargo:rerun-if-env-changed=FORCE_CATALOG_REFRESH");
    println!("cargo:rerun-if-changed=models.conf"); // Legacy file - rerun if it changes

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let out_path = PathBuf::from(&out_dir).join(CATALOG_FILENAME);
    let cache_path = PathBuf::from(&manifest_dir).join(CATALOG_FILENAME);
    let legacy_path = PathBuf::from(&manifest_dir).join("models.conf");

    // Check if we should force refresh
    let force_refresh = env::var("FORCE_CATALOG_REFRESH")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    // Check if cache is fresh (< 24 hours old)
    let cache_is_fresh = !force_refresh && is_cache_fresh(&cache_path);

    let catalog_content = if cache_is_fresh {
        println!("cargo:warning=Using cached models catalog (< 24h old)");
        read_cached_catalog(&cache_path, &legacy_path)
    } else {
        // Try to download fresh catalog
        match download_catalog() {
            Ok(content) => {
                println!("cargo:warning=Downloaded fresh models catalog from models.dev");
                // Update cache
                if let Err(e) = update_cache(&cache_path, &content) {
                    println!("cargo:warning=Failed to update cache: {}", e);
                }
                Some(content)
            }
            Err(e) => {
                println!("cargo:warning=Failed to download catalog: {}", e);
                println!("cargo:warning=Falling back to cached version");
                read_cached_catalog(&cache_path, &legacy_path)
            }
        }
    };

    // Write to OUT_DIR for include_str!
    match catalog_content {
        Some(content) => {
            write_catalog(&out_path, &content).expect("Failed to write catalog to OUT_DIR");
            println!(
                "cargo:warning=Models catalog written to {}",
                out_path.display()
            );
        }
        None => {
            // No catalog available - create a minimal fallback
            let fallback = create_fallback_catalog();
            println!("cargo:warning=No catalog available, using minimal fallback");
            write_catalog(&out_path, &fallback).expect("Failed to write fallback catalog");
        }
    }
}

/// Check if the cache file exists and is less than 24 hours old.
fn is_cache_fresh(cache_path: &Path) -> bool {
    if !cache_path.exists() {
        return false;
    }

    match fs::metadata(cache_path) {
        Ok(metadata) => {
            match metadata.modified() {
                Ok(modified) => {
                    match SystemTime::now().duration_since(modified) {
                        Ok(age) => age.as_secs() < CACHE_MAX_AGE_SECS,
                        Err(_) => false, // Clock went backwards, consider stale
                    }
                }
                Err(_) => false, // Can't get mtime, consider stale
            }
        }
        Err(_) => false,
    }
}

/// Download the catalog from models.dev with timeout.
fn download_catalog() -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(CATALOG_URL)
        .send()
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} from {}", response.status(), CATALOG_URL));
    }

    response
        .text()
        .map_err(|e| format!("Failed to read response body: {}", e))
}

/// Read cached catalog, trying cache first, then legacy models.conf.
fn read_cached_catalog(cache_path: &Path, legacy_path: &Path) -> Option<String> {
    // Try the new cache location first
    if cache_path.exists() {
        if let Ok(content) = fs::read_to_string(cache_path) {
            if !content.is_empty() {
                return Some(content);
            }
        }
    }

    // Fall back to legacy models.conf
    if legacy_path.exists() {
        if let Ok(content) = fs::read_to_string(legacy_path) {
            if !content.is_empty() {
                println!("cargo:warning=Using legacy models.conf as fallback");
                return Some(content);
            }
        }
    }

    None
}

/// Update the cache file with new content.
fn update_cache(cache_path: &Path, content: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(cache_path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Write catalog content to the output path.
fn write_catalog(out_path: &Path, content: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(out_path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Create a minimal fallback catalog when nothing else is available.
fn create_fallback_catalog() -> String {
    r#"{
  "openai": {
    "id": "openai",
    "name": "OpenAI",
    "env": ["OPENAI_API_KEY"],
    "api": "https://api.openai.com/v1",
    "doc": "https://platform.openai.com/docs",
    "models": {
      "gpt-4o": {
        "id": "gpt-4o",
        "name": "GPT-4o",
        "context_length": 128000
      },
      "gpt-4o-mini": {
        "id": "gpt-4o-mini",
        "name": "GPT-4o mini",
        "context_length": 128000
      }
    }
  }
}"#
    .to_string()
}
