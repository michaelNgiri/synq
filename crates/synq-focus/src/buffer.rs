//! Sticky buffer — prevents accidental focus switches at screen edges.
//!
//! The buffer creates a small "dead zone" at the screen boundary where
//! the cursor must dwell for a configured duration before a switch triggers.

/// Buffer zone configuration.
#[derive(Debug, Clone)]
pub struct StickyBuffer {
    /// Width of the buffer zone in pixels.
    pub width_px: u32,
    /// Dwell time required before triggering (milliseconds).
    pub dwell_ms: u64,
}

impl Default for StickyBuffer {
    fn default() -> Self {
        Self {
            width_px: 5,
            dwell_ms: 150,
        }
    }
}

impl StickyBuffer {
    /// Create a buffer with custom pixel width and dwell time.
    pub fn new(width_px: u32, dwell_ms: u64) -> Self {
        Self { width_px, dwell_ms }
    }
}
