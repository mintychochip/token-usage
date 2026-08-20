# Task 2 Report: CalVer Release Workflow

## Status

Implemented `.github/workflows/release.yml` for manually triggered CalVer releases.

## Changes

- Added a `workflow_dispatch` trigger and repository-level `contents: write` permission.
- Added `compute-version`, which computes the UTC CalVer version `YYYY.M.D.<run_number>` and exports both `version` and `tag` (`vYYYY.M.D.<run_number>`) as job outputs.
- Added the six-target `build-matrix` for Linux, macOS, and Windows targets exactly as specified.
- Configured native Cargo builds for supported targets and `cross` for `aarch64-unknown-linux-gnu`.
- Packaged both CLI binaries together with `README.md` and `LICENSE` into target-specific `.tar.gz` or `.zip` archives.
- Uploaded each archive as a uniquely named artifact.
- Added `publish`, which downloads all six archives, writes `SHA256SUMS.txt`, and publishes a GitHub release with `softprops/action-gh-release@v2`.

## Verification

- Parsed the workflow with Ruby's YAML parser using `YAML.safe_load_file`; parsing succeeded.
- Confirmed all six required target triples, archive formats, binary names, metadata files, CalVer outputs, checksum generation, and release attachment configuration by inspection.

## Commit

The workflow and this report are committed with:

`ci: add automated CalVer release workflow for 6 target platforms`
