//! macOS input engine using CoreGraphics CGEvent API (Tier 1).
//!
//! This implementation posts events at `CGEventTapLocation::HID` level,
//! which is the lowest injection point available without a kernel extension.
//! Requires Accessibility permissions in System Settings.

use core_graphics::display::CGPoint;
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use tracing::{debug, error};

use synq_core::{InputEvent, InputEventKind, MouseButton, SynqError, SynqResult};

use crate::killswitch;
use crate::InputEngine;

/// macOS input engine backed by CoreGraphics CGEvent.
///
/// Note: `CGEventSource` contains a `NonNull` pointer that is not `Send`/`Sync`.
/// We wrap it in an unsafe impl because CGEvent posting is thread-safe when
/// using `HIDSystemState` — the OS serializes all HID events regardless.
pub struct MacOSInputEngine {
    source: CGEventSource,
}

// SAFETY: CGEventSource with HIDSystemState is safe to use across threads.
// The OS HID event queue serializes all posted events.
unsafe impl Send for MacOSInputEngine {}
unsafe impl Sync for MacOSInputEngine {}

impl MacOSInputEngine {
    /// Create a new macOS input engine.
    ///
    /// Uses `HIDSystemState` as the event source, which is required for
    /// HID-level event posting.
    pub fn new() -> SynqResult<Self> {
        let source =
            CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
                SynqError::InputInjection("Failed to create CGEventSource".into())
            })?;
        Ok(Self { source })
    }

    /// Post a CGEvent at the HID tap location.
    fn post_event(&self, event: &CGEvent) {
        event.post(CGEventTapLocation::HID);
    }
}

impl InputEngine for MacOSInputEngine {
    fn inject_event(&self, event: &InputEvent) -> SynqResult<()> {
        // Safety check — refuse if kill-switch is active
        killswitch::check()?;

        match &event.kind {
            InputEventKind::MouseMove { dx, dy } => {
                let cg_event = CGEvent::new_mouse_event(
                    self.source.clone(),
                    CGEventType::MouseMoved,
                    CGPoint::new(*dx as f64, *dy as f64),
                    CGMouseButton::Left,
                )
                .map_err(|_| SynqError::InputInjection("Failed to create mouse move event".into()))?;

                self.post_event(&cg_event);
                debug!(dx, dy, "Injected mouse move");
                Ok(())
            }

            InputEventKind::MouseMoveTo { x, y } => {
                let cg_event = CGEvent::new_mouse_event(
                    self.source.clone(),
                    CGEventType::MouseMoved,
                    CGPoint::new(*x as f64, *y as f64),
                    CGMouseButton::Left,
                )
                .map_err(|_| {
                    SynqError::InputInjection("Failed to create mouse move-to event".into())
                })?;

                self.post_event(&cg_event);
                debug!(x, y, "Injected mouse move-to");
                Ok(())
            }

            InputEventKind::MouseButton { button, pressed } => {
                let (event_type, cg_button) = match (button, pressed) {
                    (MouseButton::Left, true) => {
                        (CGEventType::LeftMouseDown, CGMouseButton::Left)
                    }
                    (MouseButton::Left, false) => {
                        (CGEventType::LeftMouseUp, CGMouseButton::Left)
                    }
                    (MouseButton::Right, true) => {
                        (CGEventType::RightMouseDown, CGMouseButton::Right)
                    }
                    (MouseButton::Right, false) => {
                        (CGEventType::RightMouseUp, CGMouseButton::Right)
                    }
                    (MouseButton::Middle, true) => {
                        (CGEventType::OtherMouseDown, CGMouseButton::Center)
                    }
                    (MouseButton::Middle, false) => {
                        (CGEventType::OtherMouseUp, CGMouseButton::Center)
                    }
                    // Back/Forward are OtherMouse with different button numbers
                    _ => {
                        let et = if *pressed {
                            CGEventType::OtherMouseDown
                        } else {
                            CGEventType::OtherMouseUp
                        };
                        (et, CGMouseButton::Center)
                    }
                };

                // We need a position for mouse button events — use (0,0) as we
                // only care about the button state, not position.
                let cg_event = CGEvent::new_mouse_event(
                    self.source.clone(),
                    event_type,
                    CGPoint::new(0.0, 0.0),
                    cg_button,
                )
                .map_err(|_| {
                    SynqError::InputInjection("Failed to create mouse button event".into())
                })?;

                self.post_event(&cg_event);
                debug!(?button, pressed, "Injected mouse button");
                Ok(())
            }

            InputEventKind::Key {
                keycode, pressed, ..
            } => {
                let cg_event = CGEvent::new_keyboard_event(
                    self.source.clone(),
                    *keycode as CGKeyCode,
                    *pressed,
                )
                .map_err(|_| {
                    SynqError::InputInjection("Failed to create keyboard event".into())
                })?;

                self.post_event(&cg_event);
                debug!(keycode, pressed, "Injected key event");
                Ok(())
            }

            InputEventKind::Scroll { dx, dy } => {
                // CGEvent doesn't have a direct new_scroll_event in this crate version.
                // Use a generic CGEvent and set scroll wheel fields manually.
                let cg_event = CGEvent::new(self.source.clone())
                    .map_err(|_| SynqError::InputInjection("Failed to create scroll event".into()))?;

                cg_event.set_type(CGEventType::ScrollWheel);

                // Raw CoreGraphics field constants:
                // kCGScrollWheelEventDeltaAxis1 = 11 (vertical scroll)
                // kCGScrollWheelEventDeltaAxis2 = 12 (horizontal scroll)
                const SCROLL_WHEEL_DELTA_AXIS1: u32 = 11;
                const SCROLL_WHEEL_DELTA_AXIS2: u32 = 12;

                cg_event.set_integer_value_field(SCROLL_WHEEL_DELTA_AXIS1, *dy as i64);
                cg_event.set_integer_value_field(SCROLL_WHEEL_DELTA_AXIS2, *dx as i64);

                self.post_event(&cg_event);
                debug!(dx, dy, "Injected scroll");
                Ok(())
            }
        }
    }

    fn grab_input(&self) -> SynqResult<()> {
        // Phase 1: Implemented via CGEventTap in a later iteration.
        // For now, this is a no-op — input forwarding is handled at the
        // focus arbiter level by intercepting events before they reach apps.
        tracing::info!("Input grab requested (macOS) — placeholder");
        Ok(())
    }

    fn release_input(&self) -> SynqResult<()> {
        tracing::info!("Input release requested (macOS) — placeholder");
        Ok(())
    }

    fn emergency_kill(&self) {
        killswitch::activate();
        // Best-effort release
        let _ = self.release_input();
    }

    fn check_permissions(&self) -> SynqResult<bool> {
        // Try creating a test event — if it fails, permissions aren't granted
        let test = CGEvent::new_keyboard_event(self.source.clone(), 0, false);
        match test {
            Ok(_) => Ok(true),
            Err(_) => {
                error!("Accessibility permissions not granted — go to System Settings > Privacy & Security > Accessibility");
                Ok(false)
            }
        }
    }
}
