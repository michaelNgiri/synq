//! Unified error types for the Synq ecosystem.

use thiserror::Error;

/// Top-level error type used across all Synq crates.
#[derive(Debug, Error)]
pub enum SynqError {
    // ── Input Engine ──
    #[error("Input injection failed: {0}")]
    InputInjection(String),

    #[error("Input grab/release failed: {0}")]
    InputCapture(String),

    #[error("Kill-switch activated")]
    KillSwitch,

    #[error("Accessibility permissions not granted")]
    PermissionDenied,

    // ── Networking ──
    #[error("Peer discovery failed: {0}")]
    Discovery(String),

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Handshake failed: {0}")]
    Handshake(String),

    #[error("Send failed: {0}")]
    Send(String),

    #[error("Receive failed: {0}")]
    Receive(String),

    #[error("Peer disconnected")]
    Disconnected,

    // ── Clipboard ──
    #[error("Clipboard access failed: {0}")]
    Clipboard(String),

    #[error("CRDT sync error: {0}")]
    CrdtSync(String),

    // ── Focus ──
    #[error("Screen geometry unavailable: {0}")]
    ScreenGeometry(String),

    #[error("Focus switch failed: {0}")]
    FocusSwitch(String),

    // ── Config ──
    #[error("Configuration error: {0}")]
    Config(String),

    // ── Generic ──
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

/// Convenience alias used across the workspace.
pub type SynqResult<T> = Result<T, SynqError>;
