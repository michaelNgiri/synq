use std::thread;
use std::time::Duration;
use synq_core::{CursorPosition, DeviceId, Edge, ScreenGeometry};
use synq_focus::{create_focus_arbiter, EdgeMapping, FocusConfig};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    println!("🎯 Starting Synq Focus Arbiter Verification");
    
    let local_screen = ScreenGeometry { width: 1920, height: 1080, x: 0, y: 0 };
    let remote_screen = ScreenGeometry { width: 2560, height: 1440, x: 0, y: 0 };
    let remote_device = DeviceId::new();
    
    let config = FocusConfig {
        buffer_pixels: 5,
        buffer_dwell_ms: 150,
        edge_map: vec![EdgeMapping {
            edge: Edge::Right,
            target_device: remote_device,
            target_screen: remote_screen,
        }],
    };
    
    let mut arbiter = create_focus_arbiter(config.clone());
    
    println!("⚙️ Configuration: Sticky buffer {}px, dwell {}ms", config.buffer_pixels, config.buffer_dwell_ms);
    println!("🗺️ Mapping: Right edge -> Device {}", remote_device);
    
    // Test 1: Normal movement, not at edge
    println!("\n▶️ Test 1: Normal movement");
    let pos = CursorPosition { x: 1000, y: 500 };
    let switch = arbiter.check_edge(pos, &local_screen);
    assert!(switch.is_none(), "Should not trigger switch in the middle of the screen");
    println!("✅ Passed");
    
    // Test 2: Enter buffer zone but leave before dwell time
    println!("\n▶️ Test 2: Enter buffer zone, leave quickly");
    let pos1 = CursorPosition { x: 1918, y: 500 }; // In buffer zone (1920 - 5 = 1915)
    let switch1 = arbiter.check_edge(pos1, &local_screen);
    assert!(switch1.is_none(), "Should not trigger immediately");
    
    thread::sleep(Duration::from_millis(50));
    
    let pos2 = CursorPosition { x: 1900, y: 500 }; // Out of buffer zone
    let switch2 = arbiter.check_edge(pos2, &local_screen);
    assert!(switch2.is_none(), "Should cancel switch if left early");
    println!("✅ Passed");
    
    // Test 3: Enter buffer zone and stay for dwell time
    println!("\n▶️ Test 3: Enter buffer zone and dwell");
    let pos1 = CursorPosition { x: 1918, y: 540 }; // Center of right edge
    let switch1 = arbiter.check_edge(pos1, &local_screen);
    assert!(switch1.is_none(), "Should not trigger immediately");
    
    thread::sleep(Duration::from_millis(160)); // Wait slightly longer than dwell time
    
    let switch2 = arbiter.check_edge(pos1, &local_screen);
    assert!(switch2.is_some(), "Should trigger switch after dwell time");
    
    let command = switch2.unwrap();
    println!("🎯 Switch Triggered: {:?}", command);
    assert_eq!(command.exit_edge, Edge::Right);
    assert_eq!(command.target_device, remote_device);
    // Y should be mapped proportionally: 540 / 1080 = 0.5 -> 0.5 * 1440 = 720
    assert_eq!(command.warp_position.y, 720);
    // X should be at the opposite edge (left edge of target screen, so 0)
    assert_eq!(command.warp_position.x, 0);
    println!("✅ Passed");
    
    println!("\n🏁 All focus arbiter tests passed!");
    Ok(())
}
