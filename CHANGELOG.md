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
- `UpdateQ` command (11) — matches the server's queue-config update command. Payload is a `NetQueueConfig` binary blob (flags byte + optional `auto_fail` bool + optional `fail_timeout` u64 BE).
- `MessageMeta` Python class with fields `id`, `publisher_id`, `timestamp`, `locked_by` — parsed from the server's 56-byte Meta format.
- Client-side parsing of server Meta on Dequeue and ListM responses.
- `BrokerClient` now carries a `client_id: u128` generated at connect time (system clock based).
- New request commands mirroring the server: `CreateQ (3)`, `DeleteQ (4)`, `ListM (5)`, `DeleteM (6)`, `Succeeded (7)`, `Failed (8)`, `Requeue (9)`, `UpdateM (10)`.
- Queue name is null-padded to 64 bytes on the wire as required by the server.

### Fixed
- `BrokerClient::send` and `receive` now take `&self` instead of consuming `self`, eliminating unnecessary clones in `client_send`.
- `parse_dequeue_response` now validates payload length (>= 56 bytes) and returns `Result` instead of panicking on short responses.
- `client_send` no longer clones `BrokerClient`; it borrows through the held mutex guard.
- `connect()` now includes the original error message instead of a generic "connection failed".
- Dequeue parse failure now raises a Python exception instead of silently returning raw bytes.
- `Response::from_u8` no longer panics on unknown status bytes; returns a proper error instead.
- `receive()` no longer spins infinitely on server disconnect; detects EOF and returns `UnexpectedEof`.
- `client_id` generation uses a single `SystemTime::now()` call to avoid clock-tick race.
- `client_send` holds the outer `BrokerClient` lock across send+receive to prevent concurrent call interleaving.
- `send()` now propagates the original I/O error instead of swallowing it.
- `client_connect` uses idiomatic `match` instead of fragile `is_ok()` + `?` pattern.

## [0.2.1]

- Previous release.
