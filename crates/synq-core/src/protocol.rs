//! Wire protocol messages exchanged between Synq peers.

use serde::{Deserialize, Serialize};

use crate::types::{
    ClipboardObject, CursorPosition, DeviceId, FocusSwitchCommand, InputEvent, PeerInfo,
};

/// Top-level protocol message envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynqMessage {
    /// Sender device.
    pub from: DeviceId,
    /// Message payload.
    pub payload: MessagePayload,
    /// Sequence number for ordering (input events only).
    pub seq: u64,
}

/// All message types exchanged between peers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    // ── Handshake / Pairing ──
    /// Initial hello during discovery.
    Hello(PeerInfo),
    /// Acknowledgement of a peer's Hello.
    HelloAck(PeerInfo),

    // ── Input ──
    /// A batch of input events (batched for efficiency).
    InputBatch(Vec<InputEvent>),

    // ── Focus ──
    /// Request focus transfer to a peer.
    FocusSwitch(FocusSwitchCommand),
    /// Acknowledge focus receipt + confirm cursor warp position.
    FocusAck(CursorPosition),

    // ── Clipboard ──
    /// A new clipboard entry to be staged on the remote.
    ClipboardUpdate(ClipboardObject),
    /// Request the remote apply the staged clipboard (on paste/focus).
    ClipboardApply { entry_id: uuid::Uuid },

    // ── Control ──
    /// Heartbeat / keepalive.
    Ping(u64),
    /// Heartbeat response.
    Pong(u64),

    /// Graceful disconnect notification.
    Bye,
}
