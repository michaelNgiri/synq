# Synq

> Cross-platform input & clipboard continuity — make two machines feel like one.

## Overview

Synq is a continuity layer that enables seamless mouse, keyboard, and clipboard sharing between macOS and Windows machines over a local network. Input and clipboard sharing feels local, instant, and invisible.

## Architecture

See [architecture.md](./architecture.md) for the full implementation plan.

### Crate Structure

| Crate | Purpose |
|---|---|
| `synq-core` | Shared types, configuration, error handling |
| `synq-input` | Input engine — HID injection (CGEvent/SendInput) |
| `synq-clipboard` | CRDT-based clipboard synchronization |
| `synq-net` | WebRTC DataChannels + Noise Protocol E2EE |
| `synq-focus` | Focus arbitration — screen edge detection & cursor warp |

### Key Constraints

- **Latency:** <20ms end-to-end for input events
- **Privacy:** Zero-knowledge; all traffic E2EE via Noise Protocol
- **Reliability:** Sub-second reconnection; survives Wi-Fi toggles

## Development

```bash
# Check all crates
cargo check --workspace

# Run tests
cargo test --workspace

# Build for macOS
cargo build --workspace

# Clippy
cargo clippy --workspace -- -D warnings
```

## License

MIT
