use serde::{Deserialize, Serialize};
use synq_core::DeviceId;

/// Messages used to negotiate a WebRTC connection over the signaling channel (UDP).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignalingMessage {
    /// Initial connection request with WebRTC Offer
    Offer {
        sdp: String,
        from: DeviceId,
    },
    /// Response to an offer with WebRTC Answer
    Answer {
        sdp: String,
        from: DeviceId,
    },
    /// Network routing candidate
    IceCandidate {
        candidate: String,
        from: DeviceId,
    },
}
