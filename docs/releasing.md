# Preparing a source release

Use the pinned toolchain and a clean, reviewed commit. Run:

```sh
scripts/check.sh
scripts/audit.sh
cargo run --locked --release --example context_probe
./target/release/s1code eval --suite fixtures/core
./target/release/s1code eval --suite fixtures/heldout --output eval-results/heldout
python3 scripts/source_archive.py
```

The source packager requires a clean tree, scans source and reachable Git history,
then archives only committed files. It rejects links/private artifact directories,
checks required license files and writes a reproducible gzip archive, SHA-256
checksum and commit manifest under `target/dist/`. Ignored sessions, credentials,
local evaluation output, binaries and Git internals are not bundled. The scan is
best effort; inspect the archive before distributing it. The source archive has no
dependencies prebuilt or vendored; installation still requires Cargo's registry.

`scripts/check.sh` includes offline Rust tests, launcher checks and real terminal
and headless fixtures. Protocol fixtures and offline demos are not live-provider
acceptance. Check `docs/BUILD_STATUS.md` for outstanding validation and verify the
hosted checks for the exact commit before describing CI as passing.

Live checks require a disposable workspace, credentials and an explicit spend
budget. Keep restricted service results private and obtain applicable clearance
before sharing performance claims. Do not ship private traces as demo assets.

For binary redistribution, run `scripts/dependency_licenses.py` on each target and
include `target/notices/THIRD_PARTY_LICENSES.txt`, the project LICENSE, version and
checksum. macOS and Linux are the current targets; Windows is unsupported.

These commands do not publish a package, create a hosted release, upload artifacts
or tag a commit. Keep `publish = false` until a registry release is intentionally
prepared. Describe this version as an experimental release candidate, with its
execution and provider limitations visible to users.
