#!/bin/sh
# Host wrapper: pipe the hook payload to the Rust reporter. Do not parse usage here.
set -eu
REPORTER="${TOKEN_USAGE_REPORTER:-token-usage-reporter}"
exec "$REPORTER" ingest --adapter droid
