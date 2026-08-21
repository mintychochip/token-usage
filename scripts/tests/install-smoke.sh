#!/bin/sh
# End-to-end smoke test: install, ingest, list, publish, pull.
set -eu

BIN_DIR="${TOKTALLY_BIN_DIR:-./target/release}"
SMOKE="${TOKTALLY_SMOKE_DIR:-$(mktemp -d)}"
cleanup() { rm -rf "$SMOKE"; }
trap cleanup EXIT

# Install into a temporary prefix from built binaries.
TOKTALLY_BIN_DIR="$BIN_DIR" TOKTALLY_SKIP_BUILD=1 \
  ./scripts/install.sh --prefix "$SMOKE"

export PATH="$SMOKE/bin:$PATH"
export TOKTALLY_STORE="$SMOKE/store.json"
export TOKTALLY_HARNESS_HOME="$SMOKE"

toktally ingest --adapter hermes --file crates/adapters/fixtures/hermes-session.json

toktally list > "$SMOKE/list.json"
grep -qE '"harness"\s*:\s*"hermes"' "$SMOKE/list.json"

PUB="$SMOKE/publish"
toktally publish --dir "$PUB" --url "https://example.io/usage"

test -f "$PUB/usage-summary.json"
test -f "$PUB/usage-badge.json"

# Remove the local store and restore from the published directory.
rm "$TOKTALLY_STORE"
toktally pull --dir "$PUB"
toktally list > "$SMOKE/list2.json"
grep -qE '"harness"\s*:\s*"hermes"' "$SMOKE/list2.json"

echo "install-smoke: OK"
