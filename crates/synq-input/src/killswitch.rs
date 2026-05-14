//! Emergency kill-switch for the input engine.
//!
//! A global atomic flag that immediately halts all input injection
//! when activated. Checked before every `inject_event()` call.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use rdev::{listen, EventType, Key};

/// Global kill-switch state. When `true`, all injection is refused.
static KILL_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Activate the kill-switch. All subsequent `inject_event()` calls
/// will return `Err(SynqError::KillSwitch)`.
pub fn activate() {
    if !is_active() {
        tracing::warn!("🛑 Kill-switch ACTIVATED — all input injection halted");
        KILL_ACTIVE.store(true, Ordering::SeqCst);
    }
}

/// Deactivate the kill-switch, allowing injection to resume.
pub fn deactivate() {
    if is_active() {
        tracing::info!("✅ Kill-switch deactivated — input injection resumed");
        KILL_ACTIVE.store(false, Ordering::SeqCst);
    }
}

/// Check if the kill-switch is currently active.
pub fn is_active() -> bool {
    KILL_ACTIVE.load(Ordering::SeqCst)
}

/// Guard that should be checked at the start of every `inject_event()`.
/// Returns `Err(SynqError::KillSwitch)` if the switch is active.
pub fn check() -> synq_core::SynqResult<()> {
    if is_active() {
        Err(synq_core::SynqError::KillSwitch)
    } else {
        Ok(())
    }
}

/// Start a background thread that listens for the emergency kill-switch hotkey.
///
/// Hotkey: Ctrl + Shift + Escape
pub fn start_hotkey_listener() {
    // Permission check for macOS
    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::CGEvent;
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
        let allowed = source.is_ok() && CGEvent::new_keyboard_event(source.unwrap(), 0, false).is_ok();
        if !allowed {
            tracing::warn!("Accessibility permissions not granted. Kill-switch hotkey will NOT be active.");
            return;
        }
    }

    thread::spawn(|| {
        let mut ctrl = false;
        let mut shift = false;

        if let Err(error) = listen(move |event| {
            match event.event_type {
                EventType::KeyPress(key) => {
                    match key {
                        Key::ControlLeft | Key::ControlRight => ctrl = true,
                        Key::ShiftLeft | Key::ShiftRight => shift = true,
                        Key::Escape => {
                            if ctrl && shift {
                                activate();
                            }
                        }
                        _ => {}
                    }
                }
                EventType::KeyRelease(key) => {
                    match key {
                        Key::ControlLeft | Key::ControlRight => ctrl = false,
                        Key::ShiftLeft | Key::ShiftRight => shift = false,
                        _ => {}
                    }
                }
                _ => {}
            }
        }) {
            tracing::error!("Error listening for kill-switch hotkey: {:?}", error);
        }
    });
    
    tracing::info!("⌨️  Emergency kill-switch listener started (Ctrl+Shift+Escape)");
}
