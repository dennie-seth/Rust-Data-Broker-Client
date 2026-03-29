# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust library compiled as a Python extension module (cdylib) that implements an async TCP Data Broker Client. Python users import it as `data_broker_client` and call `connect(address)` / `send(client, path, queue_name)`.

## Build Commands

```bash
# Standard Rust build
cargo build --release

# Build Python wheel (requires maturin)
maturin build --release --skip-auditwheel -m Cargo.toml

# Run tests
cargo test

# Run the binary demo (connects to default 127.0.0.1:8080)
cargo run
# Run with a custom address
cargo run -- --address 192.168.1.5:9000
```

## Architecture

**Python FFI Layer (`src/lib.rs`)**
- Defines the `data_broker_client` Python module via PyO3
- Exports two async Python functions: `connect(url)` and `send(client, path, queue_name)`
- Manages a single static Tokio runtime via `OnceLock` shared across all Python calls
- Uses `pyo3-asyncio` to bridge Tokio futures into Python coroutines

**Core Client (`src/net/client.rs`)**
- `BrokerClient`: wraps a TCP stream, a `HashSet` of sent file paths, and a `client_id: u128`; wrapped in `Arc<Mutex<>>` for thread safety
- `PyBrokerClient`: thin PyO3-visible wrapper around `BrokerClient`
- `client_send()` spawns two concurrent Tokio tasks — one sends the file, one receives responses — both running against the same connection
- Binary protocol (big-endian, matches DataBroker server):
  - Request: `[1 byte command][16 bytes client_id u128 BE][8 bytes payload_size u64 BE][64 bytes queue_name null-padded][payload]`
  - Response: `[1 byte status][8 bytes payload_size u64 BE][payload]`
  - Request commands: `Enqueue = 1`, `Dequeue = 2`, `CreateQ = 3`, `DeleteQ = 4`, `PeekM = 5`, `DeleteM = 6`, `Succeeded = 7`, `Failed = 8`, `Requeue = 9`, `UpdateM = 10`
  - Response codes: `Succeeded = 1`, `Failed = 2`
- `client_id` is generated at connect time from the system clock (secs << 64 | subsec_nanos)
- `receive()` guards `buffer.len() >= 9 + payload_size` before calling `parse_message`, so `parse_message` always has a complete frame in the buffer

**CI/CD (`.github/workflows/`)**
- `build.yml`: builds wheels for Python 3.10/3.11/3.12 on Ubuntu and Windows on every push/PR
- `build-release.yml`: same matrix but also creates a GitHub release tagged with the version from `Cargo.toml`; requires `GH_RELEASE_TOKEN` secret