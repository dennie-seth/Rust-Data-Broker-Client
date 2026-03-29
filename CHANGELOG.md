# Changelog

## [Unreleased]

### Breaking Changes
- `send(client, path)` → `send(client, path, queue_name)`: Python callers must now pass the target queue name as a third argument.
- Request frame format updated to match DataBroker server protocol. Old frame: `[1b cmd][8b payload_size][payload]`. New frame: `[1b cmd][16b client_id][8b payload_size][64b queue_name][payload]`.

### Added
- `BrokerClient` now carries a `client_id: u128` generated at connect time (system clock based).
- New request commands mirroring the server: `CreateQ (3)`, `DeleteQ (4)`, `PeekM (5)`, `DeleteM (6)`, `Succeeded (7)`, `Failed (8)`, `Requeue (9)`, `UpdateM (10)`.
- Queue name is null-padded to 64 bytes on the wire as required by the server.

## [0.2.1]

- Previous release.
