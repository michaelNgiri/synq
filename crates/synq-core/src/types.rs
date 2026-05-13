//! Core domain types shared across all Synq crates.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─────────────────────────────────────────────
// Device Identity
// ─────────────────────────────────────────────

/// Unique identifier for a Synq device on the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub Uuid);

impl DeviceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0.to_string()[..8])
    }
}

/// Metadata about a peer device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub device_id: DeviceId,
    pub name: String,
    pub platform: Platform,
    pub screen: ScreenGeometry,
    /// Address for direct connection (IP:port or mDNS name).
    pub address: Option<String>,
}

/// Supported platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    MacOS,
    Windows,
}

// ─────────────────────────────────────────────
// Screen Geometry
// ─────────────────────────────────────────────

/// Describes the pixel dimensions and position of a display.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScreenGeometry {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// X origin of the display in the global coordinate space.
    pub x: i32,
    /// Y origin of the display in the global coordinate space.
    pub y: i32,
}

// ─────────────────────────────────────────────
// Input Events
// ─────────────────────────────────────────────

/// A single input event to be injected on a remote machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    /// What kind of input this is.
    pub kind: InputEventKind,
    /// Microsecond timestamp (from `std::time::Instant` epoch).
    pub timestamp_us: u64,
}

/// The specific input action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEventKind {
    /// Relative mouse movement.
    MouseMove { dx: i32, dy: i32 },
    /// Absolute mouse positioning.
    MouseMoveTo { x: i32, y: i32 },
    /// Mouse button press/release.
    MouseButton {
        button: MouseButton,
        pressed: bool,
    },
    /// Keyboard key press/release.
    Key {
        keycode: u16,
        pressed: bool,
        modifiers: Modifiers,
    },
    /// Scroll wheel.
    Scroll { dx: i32, dy: i32 },
}

/// Mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

/// Modifier key state.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool, // Cmd on macOS, Win on Windows
}

// ─────────────────────────────────────────────
// Cursor & Focus
// ─────────────────────────────────────────────

/// Absolute cursor position in screen coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorPosition {
    pub x: i32,
    pub y: i32,
}

/// Which edge of the screen was crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

/// Command issued when focus should switch to another device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusSwitchCommand {
    /// The edge that was crossed on the source screen.
    pub exit_edge: Edge,
    /// Cursor position at moment of crossing.
    pub exit_position: CursorPosition,
    /// Target device to receive focus.
    pub target_device: DeviceId,
    /// Computed warp position on the target screen.
    pub warp_position: CursorPosition,
}

// ─────────────────────────────────────────────
// Clipboard
// ─────────────────────────────────────────────

/// A clipboard entry synchronized across devices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardObject {
    /// Unique identifier for this clipboard entry.
    pub id: Uuid,
    /// Epoch milliseconds when the content was copied.
    pub timestamp_ms: u64,
    /// Device that originated this clipboard entry.
    pub origin_device: DeviceId,
    /// MIME type of the content (e.g. "text/plain", "image/png").
    pub mime_type: String,
    /// BLAKE3 hash of the payload for deduplication.
    pub payload_hash: [u8; 32],
    /// The actual clipboard content.
    pub content: ClipboardContent,
}

/// Clipboard payload variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    /// Plain text.
    Text(String),
    /// Image bytes (PNG/JPEG).
    Image(Vec<u8>),
    /// File references (paths are local to the origin device).
    Files(Vec<String>),
    /// Rich text with HTML and plain-text fallback.
    Rich { html: String, plain: String },
}

impl ClipboardObject {
    /// Create a new text clipboard entry.
    pub fn new_text(text: String, origin: DeviceId) -> Self {
        let hash = blake3::hash(text.as_bytes());
        Self {
            id: Uuid::new_v4(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            origin_device: origin,
            mime_type: "text/plain".into(),
            payload_hash: *hash.as_bytes(),
            content: ClipboardContent::Text(text),
        }
    }

    /// Create a new image clipboard entry.
    pub fn new_image(data: Vec<u8>, mime: &str, origin: DeviceId) -> Self {
        let hash = blake3::hash(&data);
        Self {
            id: Uuid::new_v4(),
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            origin_device: origin,
            mime_type: mime.into(),
            payload_hash: *hash.as_bytes(),
            content: ClipboardContent::Image(data),
        }
    }

    /// Check if two clipboard entries have the same content (by hash).
    pub fn content_matches(&self, other: &Self) -> bool {
        self.payload_hash == other.payload_hash
    }
}
