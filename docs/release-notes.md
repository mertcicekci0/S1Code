Experimental preview for trusted repositories. This is not a stable or sandboxed
automation product. See README, SECURITY.md and docs/BUILD_STATUS.md at this tag.

- Native Claude/OpenAI generation, optional Jev decisions and reversible context
  eviction; official Codex delegation and optional Claude Code MCP evidence ranking.
- Validated patches, execution approvals, saved sessions, resume and cancellation.
- Single-discovery text search and bounded streaming workspace hashes.
- Binary installation verifies archive checksums and retains dependency notices.

Binary targets: macOS ARM64 (built on macOS 15) and Linux x86_64 (built on Ubuntu
24.04, glibc 2.39 or newer). Other platforms should use the source instructions;
Windows is unsupported. macOS binaries are not Developer ID signed or notarized.
Checksums are published with the assets, not independently signed. No automated
updates or shell-profile changes are performed. Keep a previous binary for rollback.

Native generation requires a provider API key. Managed subscription login belongs
to the official clients; it is not a native API key and does not pay for Jev.
Tests and build scripts execute with the user's filesystem and network access.
No comparative cost, speed or correctness advantage is claimed.
