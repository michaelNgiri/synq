//! Noise Protocol handshake and session encryption.
//!
//! Uses the `snow` crate with pattern `Noise_XX_25519_ChaChaPoly_BLAKE2s`.
//! The XX pattern allows mutual authentication between previously unknown peers.

use snow::{Builder, HandshakeState, TransportState};
use synq_core::{SynqError, SynqResult};

/// Noise Protocol pattern string.
const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Maximum message size for Noise transport (64KB minus overhead).
const MAX_MSG_SIZE: usize = 65535;

/// An established Noise transport session for encrypting/decrypting messages.
pub struct NoiseSession {
    transport: TransportState,
}

impl NoiseSession {
    /// Encrypt a plaintext message.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> SynqResult<Vec<u8>> {
        let mut buf = vec![0u8; plaintext.len() + 16]; // 16 bytes AEAD tag
        let len = self
            .transport
            .write_message(plaintext, &mut buf)
            .map_err(|e| SynqError::Handshake(format!("Encryption failed: {e}")))?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Decrypt a ciphertext message.
    pub fn decrypt(&mut self, ciphertext: &[u8]) -> SynqResult<Vec<u8>> {
        let mut buf = vec![0u8; ciphertext.len()];
        let len = self
            .transport
            .read_message(ciphertext, &mut buf)
            .map_err(|e| SynqError::Handshake(format!("Decryption failed: {e}")))?;
        buf.truncate(len);
        Ok(buf)
    }
}

/// Manages the Noise XX handshake process.
/// 
/// The XX handshake has 3 messages:
/// 1. -> e
/// 2. <- e, ee, s, es
/// 3. -> s, se
pub struct NoiseHandshake {
    state: HandshakeState,
}

impl NoiseHandshake {
    /// Create an initiator (the device that starts the handshake).
    pub fn new_initiator(local_private_key: &[u8]) -> SynqResult<Self> {
        let state = Builder::new(NOISE_PATTERN.parse().map_err(|e| {
            SynqError::Handshake(format!("Invalid noise pattern: {e}"))
        })?)
        .local_private_key(local_private_key)
        .build_initiator()
        .map_err(|e| SynqError::Handshake(format!("Initiator build failed: {e}")))?;

        Ok(Self { state })
    }

    /// Create a responder (the device that receives the handshake).
    pub fn new_responder(local_private_key: &[u8]) -> SynqResult<Self> {
        let state = Builder::new(NOISE_PATTERN.parse().map_err(|e| {
            SynqError::Handshake(format!("Invalid noise pattern: {e}"))
        })?)
        .local_private_key(local_private_key)
        .build_responder()
        .map_err(|e| SynqError::Handshake(format!("Responder build failed: {e}")))?;

        Ok(Self { state })
    }

    /// Write the next handshake message (to send to the peer).
    pub fn write_message(&mut self, payload: &[u8]) -> SynqResult<Vec<u8>> {
        let mut buf = vec![0u8; MAX_MSG_SIZE];
        let len = self
            .state
            .write_message(payload, &mut buf)
            .map_err(|e| SynqError::Handshake(format!("Write handshake failed: {e}")))?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Read a handshake message received from the peer.
    pub fn read_message(&mut self, message: &[u8]) -> SynqResult<Vec<u8>> {
        let mut buf = vec![0u8; MAX_MSG_SIZE];
        let len = self
            .state
            .read_message(message, &mut buf)
            .map_err(|e| SynqError::Handshake(format!("Read handshake failed: {e}")))?;
        buf.truncate(len);
        Ok(buf)
    }

    /// Check if the handshake is complete.
    pub fn is_finished(&self) -> bool {
        self.state.is_handshake_finished()
    }

    /// Transition to transport mode after handshake completion.
    ///
    /// Consumes the handshake state and returns the encrypted session.
    pub fn into_transport(self) -> SynqResult<NoiseSession> {
        let transport = self
            .state
            .into_transport_mode()
            .map_err(|e| SynqError::Handshake(format!("Transport mode failed: {e}")))?;
        tracing::info!("🔐 Noise handshake complete — transport mode active");
        Ok(NoiseSession { transport })
    }

    /// Get the remote static public key (if learned during handshake).
    pub fn get_remote_static(&self) -> Option<&[u8]> {
        self.state.get_remote_static()
    }
}

/// Generate a new X25519 keypair for Noise Protocol.
pub fn generate_keypair() -> SynqResult<(Vec<u8>, Vec<u8>)> {
    let builder = Builder::new(NOISE_PATTERN.parse().map_err(|e| {
        SynqError::Handshake(format!("Invalid noise pattern: {e}"))
    })?);
    
    let keypair = builder.generate_keypair()
        .map_err(|e| SynqError::Handshake(format!("Keypair generation failed: {e}")))?;

    Ok((keypair.private, keypair.public))
}
