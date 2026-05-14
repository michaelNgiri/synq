//! macOS input engine using CoreGraphics CGEvent API (Tier 1).

use core_graphics::display::CGPoint;
use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use tracing::{debug, error, info};

use synq_core::{InputEvent, InputEventKind, MouseButton, Modifiers, SynqError, SynqResult};

use crate::killswitch;
use crate::InputEngine;

/// macOS input engine backed by CoreGraphics CGEvent.
pub struct MacOSInputEngine {
    source: CGEventSource,
}

unsafe impl Send for MacOSInputEngine {}
unsafe impl Sync for MacOSInputEngine {}

impl MacOSInputEngine {
    pub fn new() -> SynqResult<Self> {
        let source =
            CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
                SynqError::InputInjection("Failed to create CGEventSource".into())
            })?;
        Ok(Self { source })
    }

    fn post_event(&self, event: &CGEvent) {
        event.post(CGEventTapLocation::HID);
    }
}

impl InputEngine for MacOSInputEngine {
    fn inject_event(&self, event: &InputEvent) -> SynqResult<()> {
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
                    _ => {
                        let et = if *pressed {
                            CGEventType::OtherMouseDown
                        } else {
                            CGEventType::OtherMouseUp
                        };
                        (et, CGMouseButton::Center)
                    }
                };

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
                let cg_event = CGEvent::new(self.source.clone())
                    .map_err(|_| SynqError::InputInjection("Failed to create scroll event".into()))?;

                cg_event.set_type(CGEventType::ScrollWheel);

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

    fn start_capture(&self, callback: Box<dyn Fn(InputEvent) + Send + Sync>) -> SynqResult<()> {
        info!("Starting input capture (macOS)...");
        
        tokio::spawn(async move {
            if let Err(error) = rdev::listen(move |event| {
                if let Some(kind) = map_rdev_to_synq(event) {
                    callback(InputEvent {
                        kind,
                        timestamp_us: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_micros() as u64,
                    });
                }
            }) {
                error!("rdev listen error: {:?}", error);
            }
        });

        Ok(())
    }

    fn stop_capture(&self) -> SynqResult<()> {
        info!("Input capture stopped (macOS)");
        Ok(())
    }

    fn emergency_kill(&self) {
        killswitch::activate();
        let _ = self.stop_capture();
    }

    fn check_permissions(&self) -> SynqResult<bool> {
        let test = CGEvent::new_keyboard_event(self.source.clone(), 0, false);
        match test {
            Ok(_) => Ok(true),
            Err(_) => {
                error!("Accessibility permissions not granted");
                Ok(false)
            }
        }
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
            modifiers: Modifiers::default(),
        }),
        rdev::EventType::KeyRelease(key) => Some(InputEventKind::Key {
            keycode: map_rdev_key(key),
            pressed: false,
            modifiers: Modifiers::default(),
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

fn map_rdev_key(_key: rdev::Key) -> u16 {
    0 
}
