# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust library compiled as a Python extension module (cdylib) that implements an async TCP Data Broker Client. Python users import it as `data_broker_client` and call `connect(address)` / `send(client, path)`.

## Build Commands

```bash
# Standard Rust build
cargo build --release

# Build Python wheel (requires maturin)
maturin build --release --skip-auditwheel -m Cargo.toml

# Run tests
cargo test

# Run the binary demo
cargo run
```

## Architecture

**Python FFI Layer (`src/lib.rs`)**
- Defines the `data_broker_client` Python module via PyO3
- Exports two async Python functions: `connect(url)` and `send(client, path)`
- Manages a single static Tokio runtime via `OnceLock` shared across all Python calls
- Uses `pyo3-asyncio` to bridge Tokio futures into Python coroutines

**Core Client (`src/net/client.rs`)**
- `BrokerClient`: wraps a TCP stream and a `HashSet` of sent file paths; wrapped in `Arc<Mutex<>>` for thread safety
- `PyBrokerClient`: thin PyO3-visible wrapper around `BrokerClient`
- `client_send()` spawns two concurrent Tokio tasks — one sends the file, one receives responses — both running against the same connection
- Binary protocol (big-endian, matches DataBroker server):
  - Request: `[1 byte command][8 bytes payload_size u64 BE][payload]`
  - Response: `[1 byte status][8 bytes payload_size u64 BE][payload]`
  - Request commands: `Enqueue = 1`, `Dequeue = 2`
  - Response codes: `Succeeded = 1`, `Failed = 2`
- `receive()` guards `buffer.len() >= 9 + payload_size` before calling `parse_message`, so `parse_message` always has a complete frame in the buffer

**CI/CD (`.github/workflows/`)**
- `build.yml`: builds wheels for Python 3.10/3.11/3.12 on Ubuntu and Windows on every push/PR
- `build-release.yml`: same matrix but also creates a GitHub release tagged with the version from `Cargo.toml`; requires `GH_RELEASE_TOKEN` secret