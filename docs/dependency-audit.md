# Dependency and license checks

The source uses Cargo.lock and Rust 1.94.0. `scripts/dependency_licenses.py` walks the
resolved dependency graph for the current host, validates that at least one SPDX
license alternative is in its reviewed set, and assembles the original license and
notice files into `target/notices/THIRD_PARTY_LICENSES.txt`. Include that bundle
when redistributing a binary. Repeat the check for each distribution target.

The 2026-09-19 local audit used cargo-audit 0.22.2 and RustSec database commit
`2b34578f89884736e0fcbd42f7ba8d6b10b4a0ce` (updated 2026-09-18). The initial terminal
dependency introduced unmaintained `paste` and soundness warnings in `lru 0.12.5`.
Upgrading to Ratatui 0.30.2 removed `paste` and resolved `lru` to 0.18.4. The subsequent
audit reported **zero vulnerabilities and zero warnings**. This is a dated check,
not a guarantee about future advisories.

Sources: [Ratatui 0.30](https://ratatui.rs/highlights/v030/) and
[RustSec RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html).
No third-party source patches were vendored. Run `scripts/audit.sh` with cargo-audit
installed to repeat the advisory check. Source/header provenance and provider
agreements remain separate from dependency advisory scanning.
