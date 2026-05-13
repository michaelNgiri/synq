//! Automatic reconnection state machine.
//!
//! When a connection drops, this module retries at 200ms intervals
//! for up to 15 attempts (~3 seconds). Input events are buffered
//! during reconnection and replayed if connection restores within 500ms.

use std::time::{Duration, Instant};

/// Reconnection configuration.
pub struct ReconnectConfig {
    /// Interval between reconnection attempts.
    pub retry_interval: Duration,
    /// Maximum number of retry attempts before giving up.
    pub max_retries: u32,
    /// Maximum age of buffered events that will be replayed on reconnect.
    pub buffer_max_age: Duration,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            retry_interval: Duration::from_millis(200),
            max_retries: 15,
            buffer_max_age: Duration::from_millis(500),
        }
    }
}

/// Reconnection state tracker.
pub struct ReconnectState {
    config: ReconnectConfig,
    /// When the disconnection was first detected.
    disconnected_at: Option<Instant>,
    /// Current attempt count.
    attempts: u32,
}

impl ReconnectState {
    pub fn new(config: ReconnectConfig) -> Self {
        Self {
            config,
            disconnected_at: None,
            attempts: 0,
        }
    }

    /// Signal that the connection has been lost.
    pub fn on_disconnect(&mut self) {
        self.disconnected_at = Some(Instant::now());
        self.attempts = 0;
        tracing::warn!("Connection lost — starting reconnection attempts");
    }

    /// Attempt a reconnection. Returns `true` if we should keep trying.
    pub fn should_retry(&mut self) -> bool {
        if self.attempts >= self.config.max_retries {
            tracing::error!(
                "Reconnection failed after {} attempts — giving up",
                self.attempts
            );
            return false;
        }
        self.attempts += 1;
        tracing::info!(
            "Reconnection attempt {}/{}",
            self.attempts,
            self.config.max_retries
        );
        true
    }

    /// Signal that the connection has been restored.
    pub fn on_reconnect(&mut self) {
        if let Some(at) = self.disconnected_at.take() {
            let elapsed = at.elapsed();
            tracing::info!(
                "✅ Reconnected after {:?} ({} attempts)",
                elapsed,
                self.attempts
            );
        }
        self.attempts = 0;
    }

    /// Check if buffered events should be replayed (within buffer_max_age).
    pub fn should_replay_buffer(&self) -> bool {
        self.disconnected_at
            .map(|at| at.elapsed() < self.config.buffer_max_age)
            .unwrap_or(false)
    }

    /// Get the retry interval.
    pub fn retry_interval(&self) -> Duration {
        self.config.retry_interval
    }

    /// Get current attempt count.
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}
