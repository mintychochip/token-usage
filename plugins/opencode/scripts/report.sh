#!/bin/sh
# Host wrapper: pipe the hook payload to the Rust reporter. Do not parse usage here.
set -eu
REPORTER="${TOKTALLY_REPORTER:-${TOKEN_USAGE_REPORTER:-toktally}}"
exec "$REPORTER" ingest --adapter opencode
