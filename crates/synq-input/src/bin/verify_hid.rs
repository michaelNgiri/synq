use std::thread;
use std::time::Duration;
use synq_core::{InputEvent, InputEventKind};
use synq_input::{create_input_engine, killswitch};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    println!("🚀 Starting Synq HID Verification");
    println!("⚠️  WARNING: This will take control of your mouse and keyboard.");
    println!("🛑 Emergency Kill-switch: The script will stop if it detects a kill-switch state.");
    println!("⌛ Starting in 3 seconds... Move your cursor to a safe place (like a text editor).");
    
    thread::sleep(Duration::from_secs(3));
    
    killswitch::start_hotkey_listener();
    
    let engine = create_input_engine()?;
    
    if !engine.check_permissions()? {
        println!("❌ ERROR: Accessibility permissions NOT granted.");
        println!("Please go to System Settings > Privacy & Security > Accessibility and enable your terminal.");
        return Ok(());
    }
    
    println!("✅ Permissions granted. Starting injection...");

    // 1. Mouse Movement Circle
    println!("🖱️  Testing Mouse Movement (Circle)...");
    for i in 0..100 {
        if killswitch::is_active() { break; }
        
        let angle = (i as f64) * 0.1;
        let dx = (angle.cos() * 10.0) as i32;
        let dy = (angle.sin() * 10.0) as i32;
        
        engine.inject_event(&InputEvent {
            kind: InputEventKind::MouseMove { dx, dy },
            timestamp_us: 0,
        })?;
        
        thread::sleep(Duration::from_millis(10));
    }

    // 2. Typing Test
    println!("⌨️  Testing Keyboard Injection...");
    // macOS Keycodes: S=1, Y=16, N=45, Q=12, Space=49 ... 
    // This is low level, so we'd normally need a mapper. 
    // For verification, let's just do a few simple ones if we have a mapper.
    // Since we don't have a mapper yet, let's just test a single key.
    
    // Keycode 1 is 's' on macOS
    let test_keys = vec![1, 16, 45, 12]; // "synq"
    
    for &code in &test_keys {
        if killswitch::is_active() { break; }
        
        engine.inject_event(&InputEvent {
            kind: InputEventKind::Key { keycode: code, pressed: true, modifiers: Default::default() },
            timestamp_us: 0,
        })?;
        thread::sleep(Duration::from_millis(50));
        engine.inject_event(&InputEvent {
            kind: InputEventKind::Key { keycode: code, pressed: false, modifiers: Default::default() },
            timestamp_us: 0,
        })?;
        thread::sleep(Duration::from_millis(50));
    }

    // 3. Scroll Test
    println!("📜 Testing Scroll...");
    for _ in 0..10 {
        if killswitch::is_active() { break; }
        engine.inject_event(&InputEvent {
            kind: InputEventKind::Scroll { dx: 0, dy: -10 },
            timestamp_us: 0,
        })?;
        thread::sleep(Duration::from_millis(50));
    }

    println!("🏁 Verification complete!");
    Ok(())
}
