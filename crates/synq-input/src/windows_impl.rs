//! Windows input engine using Win32 SendInput API (Tier 1).

use synq_core::{InputEvent, MouseButton, InputEventKind, SynqError, SynqResult};
use tracing::{info, error};

use crate::killswitch;
use crate::InputEngine;

/// Windows input engine backed by Win32 `SendInput`.
pub struct WindowsInputEngine;

impl WindowsInputEngine {
    pub fn new() -> SynqResult<Self> {
        Ok(Self)
    }
}

impl InputEngine for WindowsInputEngine {
    fn inject_event(&self, _event: &InputEvent) -> SynqResult<()> {
        killswitch::check()?;
        // TODO: Implement SendInput injection for each InputEventKind
        tracing::warn!("Windows input injection pending full implementation");
        Ok(())
    }

    fn start_capture(&self, callback: Box<dyn Fn(InputEvent) + Send + Sync>) -> SynqResult<()> {
        info!("Starting input capture (Windows)...");
        
        tokio::spawn(async move {
            if let Err(error) = rdev::listen(move |event| {
                if let Some(kind) = map_rdev_to_synq(event) {
                    callback(InputEvent {
                        kind,
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64,
                    });
                }
            }) {
                error!("rdev listen error: {:?}", error);
            }
        });

        Ok(())
    }

    fn stop_capture(&self) -> SynqResult<()> {
        info!("Input capture stopped (Windows)");
        Ok(())
    }

    fn emergency_kill(&self) {
        killswitch::activate();
        let _ = self.stop_capture();
    }

    fn check_permissions(&self) -> SynqResult<bool> {
        Ok(true)
    }
}

fn map_rdev_to_synq(event: rdev::Event) -> Option<InputEventKind> {
    match event.event_type {
        rdev::EventType::MouseMove { x, y } => Some(InputEventKind::MouseMoveTo {
            x: x as i32,
            y: y as i32,
        }),
        rdev::EventType::ButtonPress(button) => Some(InputEventKind::MouseButton {
            button: map_rdev_button(button),
            pressed: true,
        }),
        rdev::EventType::ButtonRelease(button) => Some(InputEventKind::MouseButton {
            button: map_rdev_button(button),
            pressed: false,
        }),
        rdev::EventType::KeyPress(key) => Some(InputEventKind::Key {
            keycode: map_rdev_key(key),
            pressed: true,
            modifiers: 0,
        }),
        rdev::EventType::KeyRelease(key) => Some(InputEventKind::Key {
            keycode: map_rdev_key(key),
            pressed: false,
            modifiers: 0,
        }),
        rdev::EventType::Wheel { delta_x, delta_y } => Some(InputEventKind::Scroll {
            dx: delta_x as i32,
            dy: delta_y as i32,
        }),
    }
}

fn map_rdev_button(button: rdev::Button) -> MouseButton {
    match button {
        rdev::Button::Left => MouseButton::Left,
        rdev::Button::Right => MouseButton::Right,
        rdev::Button::Middle => MouseButton::Middle,
        _ => MouseButton::Left,
    }
}

fn map_rdev_key(_key: rdev::Key) -> u32 {
    0 
}
