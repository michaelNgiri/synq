//! Screen-edge detection with sticky buffer support.

use std::time::Instant;

use synq_core::{CursorPosition, Edge, FocusSwitchCommand, ScreenGeometry};

use crate::{EdgeMapping, FocusArbiter, FocusConfig};

/// Edge detector with sticky buffer zone.
pub struct EdgeDetector {
    config: FocusConfig,
    /// When the cursor first entered the buffer zone.
    entered_buffer_at: Option<Instant>,
    /// Which edge the cursor is currently in the buffer zone of.
    current_edge: Option<Edge>,
}

impl EdgeDetector {
    pub fn new(config: FocusConfig) -> Self {
        Self {
            config,
            entered_buffer_at: None,
            current_edge: None,
        }
    }

    /// Check if a cursor position is within the buffer zone of any mapped edge.
    fn detect_edge(&self, cursor: CursorPosition, screen: &ScreenGeometry) -> Option<Edge> {
        let buf = self.config.buffer_pixels as i32;

        // Right edge
        if cursor.x >= (screen.x + screen.width as i32 - buf) {
            if self.has_mapping(Edge::Right) {
                return Some(Edge::Right);
            }
        }
        // Left edge
        if cursor.x <= (screen.x + buf) {
            if self.has_mapping(Edge::Left) {
                return Some(Edge::Left);
            }
        }
        // Top edge
        if cursor.y <= (screen.y + buf) {
            if self.has_mapping(Edge::Top) {
                return Some(Edge::Top);
            }
        }
        // Bottom edge
        if cursor.y >= (screen.y + screen.height as i32 - buf) {
            if self.has_mapping(Edge::Bottom) {
                return Some(Edge::Bottom);
            }
        }

        None
    }

    /// Check if there's an edge mapping for the given edge.
    fn has_mapping(&self, edge: Edge) -> bool {
        self.config.edge_map.iter().any(|m| m.edge == edge)
    }

    /// Get the edge mapping for the given edge.
    fn get_mapping(&self, edge: Edge) -> Option<&EdgeMapping> {
        self.config.edge_map.iter().find(|m| m.edge == edge)
    }
}

impl FocusArbiter for EdgeDetector {
    fn check_edge(
        &mut self,
        cursor: CursorPosition,
        local_screen: &ScreenGeometry,
    ) -> Option<FocusSwitchCommand> {
        let detected = self.detect_edge(cursor, local_screen);

        match detected {
            Some(edge) => {
                if self.current_edge == Some(edge) {
                    // Still in the same buffer zone — check dwell time
                    if let Some(entered_at) = self.entered_buffer_at {
                        let dwell = entered_at.elapsed().as_millis() as u64;
                        if dwell >= self.config.buffer_dwell_ms {
                            // Dwell time exceeded — trigger focus switch!
                            let mapping = self.get_mapping(edge)?;
                            let warp = crate::warp::calculate_warp(
                                cursor,
                                edge,
                                local_screen,
                                &mapping.target_screen,
                            );

                            tracing::info!(
                                ?edge,
                                dwell_ms = dwell,
                                "🎯 Focus switch triggered"
                            );

                            return Some(FocusSwitchCommand {
                                exit_edge: edge,
                                exit_position: cursor,
                                target_device: mapping.target_device,
                                warp_position: warp,
                            });
                        }
                    }
                } else {
                    // Entered a new buffer zone
                    self.current_edge = Some(edge);
                    self.entered_buffer_at = Some(Instant::now());
                    tracing::debug!(?edge, "Cursor entered buffer zone");
                }
            }
            None => {
                // Cursor left the buffer zone — reset
                if self.current_edge.is_some() {
                    tracing::debug!("Cursor left buffer zone — cancelled");
                    self.current_edge = None;
                    self.entered_buffer_at = None;
                }
            }
        }

        None
    }

    fn reset(&mut self) {
        self.current_edge = None;
        self.entered_buffer_at = None;
    }

    fn update_config(&mut self, config: FocusConfig) {
        self.config = config;
        self.reset();
    }
}
