//! synq-focus — Focus arbitration, screen-edge detection, and cursor warp.
//!
//! This module decides when and how to transfer input control between devices
//! based on cursor position relative to screen boundaries.

pub mod buffer;
pub mod edge;
pub mod warp;

use synq_core::{CursorPosition, DeviceId, Edge, FocusSwitchCommand, ScreenGeometry};

/// Configuration for the focus arbiter.
#[derive(Debug, Clone)]
pub struct FocusConfig {
    /// Width of the sticky buffer zone in pixels.
    pub buffer_pixels: u32,
    /// Time the cursor must stay in the buffer zone before switching (ms).
    pub buffer_dwell_ms: u64,
    /// Screen arrangements — maps edges to peer devices.
    pub edge_map: Vec<EdgeMapping>,
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            buffer_pixels: 5,
            buffer_dwell_ms: 150,
            edge_map: Vec::new(),
        }
    }
}

/// Maps a screen edge to a target peer.
#[derive(Debug, Clone)]
pub struct EdgeMapping {
    pub edge: Edge,
    pub target_device: DeviceId,
    pub target_screen: ScreenGeometry,
}

/// The focus arbiter — determines when to switch input control between devices.
pub trait FocusArbiter: Send + Sync {
    /// Check if the cursor is at a screen edge that should trigger a focus switch.
    ///
    /// Returns `Some(FocusSwitchCommand)` if a switch should happen,
    /// or `None` if the cursor is in normal territory.
    fn check_edge(
        &mut self,
        cursor: CursorPosition,
        local_screen: &ScreenGeometry,
    ) -> Option<FocusSwitchCommand>;

    /// Reset the edge detection state (e.g., after a focus switch completes).
    fn reset(&mut self);

    /// Update the screen layout configuration.
    fn update_config(&mut self, config: FocusConfig);
}

/// Create the default focus arbiter.
pub fn create_focus_arbiter(config: FocusConfig) -> Box<dyn FocusArbiter> {
    Box::new(edge::EdgeDetector::new(config))
}
