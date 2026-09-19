# Dependency and license checks

The source uses Cargo.lock and Rust 1.94.0. `scripts/dependency_licenses.py` walks the
resolved dependency graph for the current host, validates that at least one SPDX
license alternative is in its reviewed set, and assembles the original license and
notice files into `target/notices/THIRD_PARTY_LICENSES.txt`. Include that bundle
when redistributing a binary. Each entry includes a version-specific source archive
URL as well as the original notices. Non-crates.io sources require explicit review.
Repeat the check for each distribution target.

`option-ext` 0.2.0 is an unmodified MPL-2.0 dependency. Its original source and
license notices are available in the [0.2.0 source archive](https://crates.io/api/v1/crates/option-ext/0.2.0/download).
Binary notice bundles include that retrieval link, following the source-availability
requirement described in [MPL section 3.2](https://www.mozilla.org/en-US/MPL/2.0/) and
[Mozilla's distribution FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/#q8-i-want-to-distribute-outside-my-organization-executable-programs-or-libraries-that-i-have-compiled-from-someone-elses-unchanged-mpl-licensed-source-code-either-standalone-or-part-of-a-larger-work-what-do-i-have-to-do).
The MPL applies to that dependency; new S1Code code remains Apache-2.0.

The 2026-09-19 local audit used cargo-audit 0.22.2 and RustSec database commit
`d5c17953a895cf19e8d3ce66eaa42b6fcfe1fb16` (updated 2026-09-19). The initial terminal
dependency introduced unmaintained `paste` and soundness warnings in `lru 0.12.5`.
Upgrading to Ratatui 0.30.2 removed `paste` and resolved `lru` to 0.18.4. The subsequent
audit reported **zero vulnerabilities and zero warnings**. This is a dated check,
not a guarantee about future advisories.

Sources: [Ratatui 0.30](https://ratatui.rs/highlights/v030/) and
[RustSec RUSTSEC-2026-0253](https://rustsec.org/advisories/RUSTSEC-2026-0253.html).
No third-party source patches were vendored. Run `scripts/audit.sh` with cargo-audit
installed to repeat the advisory check. Source/header provenance and provider
agreements remain separate from dependency advisory scanning.
