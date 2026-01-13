//! Stockpot Library
//!
//! This crate provides the core functionality for the Stockpot GUI application.

pub mod agents;
pub mod auth;
pub mod config;
pub mod db;
pub mod mcp;
pub mod messaging;
pub mod models;
pub mod session;
pub mod terminal;
pub mod tokens;
pub mod tools;
pub mod version_check;

#[cfg(feature = "gui")]
pub mod gui;

#[cfg(feature = "tui")]
pub mod tui;
