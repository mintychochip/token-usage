# Local-First Developer Experience — Plan & Verification

**Goal:** Make the shipped local-first v1 coherent for a new developer by aligning documentation, installer, and release naming with `toktally`, then verify one install → ingest → list → publish/pull path without implementing the draft widgets service.

**Verification:**

```bash
cargo build --workspace --release
./scripts/tests/install-smoke.sh
```

The smoke test exercises:

1. Install `toktally` and `toktally-api` into a temporary prefix from built binaries.
2. Ingest `crates/adapters/fixtures/hermes-session.json` into a local store.
3. `toktally list` and assert the Hermes observation is present.
4. Publish the summary and badge to a temporary directory.
5. Delete the local store and `toktally pull --dir` to restore it.
6. `toktally list` again and assert the Hermes observation is still present.

**Result:** `install-smoke: OK` (2026-08-21).

**Out of scope (draft only):**

- `toktally publish --widgets`
- Managed widget hosting
- Ed25519 machine identities
- Multi-target publish fan-out
- Central usage storage
