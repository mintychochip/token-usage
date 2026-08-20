# Store and API Security Hardening Design

## Scope

Fix two verified security and correctness defects without changing the JSON store format or adding remote authentication:

1. The unauthenticated HTTP API can be exposed on non-loopback interfaces.
2. `FileStore` coordinates only threads in one process and uses a shared temporary pathname, so concurrent processes can lose updates or interfere with writes.

## API Security Boundary

`token_usage_cli::serve` and `token_usage_cli::serve_stateless` are the enforcement boundary because they are public and callers can bypass the binary's environment parsing.

Both functions must reject any `SocketAddr` whose IP address is not loopback. Rejection must return `std::io::ErrorKind::InvalidInput` before opening a store or binding a listener. IPv4 `127.0.0.0/8` and IPv6 `::1` remain valid, including ephemeral port `0`. Wildcard, LAN, and public addresses are rejected. There is no unsafe override while the routes remain unauthenticated.

The binary continues parsing `TOKEN_USAGE_BIND`, but delegates the security decision to the library functions.

## Store Coordination

Each store uses a stable sidecar lock file derived from its store path. The data file itself cannot be the lock target because atomic replacement changes its inode and could split contenders across old and new files.

`FileStore::open` acquires the inter-process lock before checking whether the store exists and holds it through initialization. This prevents two first-openers from racing.

Every operation that reads or mutates the store acquires the existing in-process mutex and then the sidecar lock in the same order. Mutations hold both guards across the complete read-modify-write transaction. Lock guards are RAII values, so every return path releases locks automatically.

Exclusive filesystem locking is used for all operations. Shared read locks could improve concurrency, but add complexity without evidence that this local store needs it.

## Durable Atomic Replacement

Writes use a uniquely named temporary file created in the destination directory. The payload is serialized and written, the temporary file is flushed and synced, and it is atomically persisted over the store path. A stable shared `.json.tmp` pathname is forbidden.

After replacement, the parent directory is synced on supported platforms so the rename is durable across crashes. Platform-specific directory syncing must be explicitly gated where unsupported; failures on supported platforms propagate rather than silently weakening durability.

All lock, serialization, write, sync, and rename failures propagate through `StoreError`. The implementation does not delete allegedly stale locks or attempt speculative recovery.

## Dependencies

Use established crates rather than custom lock-file protocols:

- `fs2` for RAII-backed OS file locking.
- Existing workspace `tempfile` for unique same-directory temporary files, promoted from dev-only use in the store crate.

No storage migration is required. The sidecar lock contains no application data.

## Verification

API tests exercise both exported serving functions:

- IPv4 and IPv6 wildcard addresses return `InvalidInput` without binding.
- A non-loopback address returns `InvalidInput`.
- Existing loopback binary round-trip remains successful; direct ephemeral loopback coverage is added where practical without leaving servers running.
- Stateful rejection occurs before store creation.

Store tests exercise independent processes rather than only threads:

- Concurrent first-openers produce one valid initialized store.
- Coordinated concurrent ingests of distinct observations preserve both records.
- Final storage parses as valid JSON and no fixed shared temporary file is used.

Targeted crate tests run first, followed by workspace tests and compilation checks on the active platform. Directory-sync conditional compilation is verified by the workspace build; unsupported targets retain explicit gated behavior rather than an implicit ignored error.

## Non-goals

- Remote API authentication or authorization.
- CORS as a substitute for network access control.
- SQLite migration.
- Automatic repair of previously corrupted stores.
- Unrelated API or store refactoring.
