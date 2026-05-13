//! synq-core — Shared types, configuration, and error definitions for Synq.
//!
//! This crate contains all foundational types used across the Synq workspace.
//! No platform-specific code lives here.

pub mod config;
pub mod error;
pub mod protocol;
pub mod types;

// Re-export commonly used types at crate root
pub use error::{SynqError, SynqResult};
pub use types::*;
