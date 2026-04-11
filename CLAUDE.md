# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust library compiled as a Python extension module (cdylib) that implements an async TCP Data Broker Client. Python users import it as `data_broker_client` and call `connect(address)` / `send(client, command, payload, queue_name)`.

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
- Exports two async Python functions: `connect(url)` and `send(client, command, payload, queue_name)`
- Manages a single static Tokio runtime via `OnceLock` shared across all Python calls
- Uses `pyo3-asyncio` to bridge Tokio futures into Python coroutines

**Core Client (`src/net/client.rs`)**
- `BrokerClient`: wraps a TCP stream and a `client_id: u128`; wrapped in `Arc<Mutex<>>` for thread safety
- `PyBrokerClient`: thin PyO3-visible wrapper around `BrokerClient`
- `MessageMeta`: PyO3-visible struct with fields `id: u128`, `publisher_id: u128`, `timestamp: u64`, `locked_by: Option<u128>` — parsed from the server's 56-byte Meta format (big-endian, `u128::MAX` → `None` for `locked_by`)
- `QueueStat`: PyO3-visible struct with fields `queue_name: String`, `total_messages: u64`, `total_bytes: u64`, `total_messages_locked: u64`, `total_bytes_locked: u64` — parsed from the server's `NetStats` response (nested `[u32 count][{u32 stat_len}{u16 name_len}{name}{4×u64 counters}]` layout, all big-endian)
- `client_send()` sends a request (any command) with a `Vec<u8>` payload, then awaits and returns the server's response payload as `Vec<u8>`
- Failed responses are parsed as 2-byte big-endian `u16` error codes matching the server's `ErrorCode` enum, and converted to human-readable error messages via `error_code_message()`
- Response parsing varies by command:
  - **Dequeue (2)**: returns `tuple(MessageMeta, bytes)` — meta separated from payload
  - **ListM (5)**: returns `list[MessageMeta]` — one entry per 56-byte chunk
  - **NetStats (12)**: returns `list[QueueStat]` — one entry per queue
  - **All others**: returns raw `bytes`
- Binary protocol (big-endian, matches DataBroker server):
  - Request: `[1 byte command][16 bytes client_id u128 BE][8 bytes payload_size u64 BE][64 bytes queue_name null-padded][payload]`
  - Response: `[1 byte status][8 bytes payload_size u64 BE][payload]`
  - Request commands: `Enqueue = 1`, `Dequeue = 2`, `CreateQ = 3`, `DeleteQ = 4`, `ListM = 5`, `DeleteM = 6`, `Succeeded = 7`, `Failed = 8`, `Requeue = 9`, `UpdateM = 10`, `UpdateQ = 11`, `NetStats = 12`
  - Response codes: `Succeeded = 1`, `Failed = 2`
- `client_id` is generated at connect time from the system clock (secs << 64 | subsec_nanos)
- `receive()` holds the stream lock for the entire read loop (not per-iteration) and guards `buffer.len() >= 9 + payload_size` before calling `parse_message`
- `payload_size` is converted from `u64` to `usize` via `try_into()` with overflow checks, safe on both 32-bit and 64-bit platforms
- Queue names are validated to not exceed 64 bytes before sending

**CI/CD (`.github/workflows/`)**
- `build.yml`: builds wheels for Python 3.10/3.11/3.12 on Ubuntu and Windows on every push/PR
- `build-release.yml`: same matrix but also creates a GitHub release tagged with the version from `Cargo.toml`; requires `GH_RELEASE_TOKEN` secret