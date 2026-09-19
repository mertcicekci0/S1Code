# Preparing a release

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

## Binary candidates

The `release candidate binaries` workflow is manually triggered from a tested
commit. Both macOS ARM64 and Linux x86_64 run the offline suite, build with compiler
path remapping, collect license notices, package the binary, install its archive
in a temporary prefix and run the real offline patch/approval/resume smoke test.
A separate advisory job must also pass. All actions are pinned to commit hashes.
Only the final draft job has repository write access; build jobs have read access.

```sh
gh workflow run release.yml --ref main
```

The final job creates an **unpublished prerelease draft**, with two binary archives,
SHA256SUMS and the installer. It refuses to overwrite an existing release. Before
publication, inspect the exact commit's checks and downloaded assets, their
manifests, notices and checksums. Use `gh release edit VERSION --draft=false` only
after that review. A source push alone does not publish a package or binary.

To reproduce host packaging locally from a clean checkout:

```sh
sh scripts/build_release.sh
python3 scripts/dependency_licenses.py
python3 scripts/binary_archive.py
```

Archive headers contain no account names, paths or timestamps. The manifest records
only version, target, source commit and binary SHA-256. The packager refuses a
binary containing the current checkout/home path. Build path remapping is not a
general secret detector; source/history scanning and clean hosted builds remain
required. The archive format is deterministic for a given binary, not a claim of
bit-for-bit reproducibility across different compilers or hosts. macOS signing,
notarization and independently signed checksums are not configured. Release notes
must disclose those limits, target OS requirements and the experimental status.

See [installation](install.md) for user commands. npm wrappers and registry
publication are unnecessary for this binary distribution and are not configured.

Workflow references: [manual dispatch](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow)
and [hosted runner targets](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), reviewed 2026-09-19.
