# Changelog

## [Unreleased]

### Breaking Changes
- `send(client, path)` → `send(client, path, queue_name)`: Python callers must now pass the target queue name as a third argument.
- Request frame format updated to match DataBroker server protocol. Old frame: `[1b cmd][8b payload_size][payload]`. New frame: `[1b cmd][16b client_id][8b payload_size][64b queue_name][payload]`.
- `send()` return type now varies by command:
  - **Dequeue (2)**: returns `tuple(MessageMeta, bytes)` instead of raw bytes.
  - **ListM (5)**: returns `list[MessageMeta]` instead of raw bytes.
  - All other commands still return `bytes`.

### Added
- `MessageMeta` Python class with fields `id`, `publisher_id`, `timestamp`, `locked_by` — parsed from the server's 56-byte Meta format.
- Client-side parsing of server Meta on Dequeue and ListM responses.
- `BrokerClient` now carries a `client_id: u128` generated at connect time (system clock based).
- New request commands mirroring the server: `CreateQ (3)`, `DeleteQ (4)`, `ListM (5)`, `DeleteM (6)`, `Succeeded (7)`, `Failed (8)`, `Requeue (9)`, `UpdateM (10)`.
- Queue name is null-padded to 64 bytes on the wire as required by the server.

### Fixed
- `Response::from_u8` no longer panics on unknown status bytes; returns a proper error instead.
- `receive()` no longer spins infinitely on server disconnect; detects EOF and returns `UnexpectedEof`.
- `client_id` generation uses a single `SystemTime::now()` call to avoid clock-tick race.
- `client_send` holds the outer `BrokerClient` lock across send+receive to prevent concurrent call interleaving.
- `send()` now propagates the original I/O error instead of swallowing it.
- `client_connect` uses idiomatic `match` instead of fragile `is_ok()` + `?` pattern.

## [0.2.1]

- Previous release.
