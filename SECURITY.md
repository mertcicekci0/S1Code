# Security

Native process execution is not an operating-system sandbox. Cargo tests/build
scripts, Python tests and Node.js tests are arbitrary repository code, executed with the user's
filesystem and network access after explicit approval. Use trusted repositories,
disposable workspaces, or an external sandbox. An allowlist, file hashes and a Git
worktree do not provide OS isolation. macOS/Linux process groups support cancellation;
a deliberately daemonizing process can escape a process group. No memory/container
isolation is claimed.

Model outputs and repository instructions are untrusted data. Native deterministic
policy is allow/ask/deny. Models cannot change it. By default, each patch/process requires an
interactive approval bound to complete arguments, policy version, workspace identity and file
snapshot. There is no automatic commit, push, install, destructive shell, or shell
interpolation tool. The small process set currently supports offline Cargo test/check,
Python unittest/pytest, exact `node --test` and the existing npm scripts documented
in [supported checks](docs/project-checks.md). Arbitrary Node flags, package
installation and shell commands remain unsupported.

Native paths reject traversal, absolute paths, symlinks, dotfiles, ignored paths,
common credential names and explicit exclusions. Root `AGENTS.md` can guide coding
but cannot grant permissions. These guards govern S1Code file tools; approved
repository code can access files outside them. The implementation assumes no hostile
concurrent filesystem actor replacing parent directories during syscalls. Ordinary
concurrent edits are checked before selection, after approval and before writes;
S1Code is not a defense against malicious same-user filesystem races.

A workspace advisory lock serializes S1Code writers across storage homes. Other
editors do not participate. All patch files are validated before mutation. Writes
are individually replaced and a durable journal retains original artifact bytes.
This is not multi-file atomicity. Recovery checks every current hash before restoring
and stops on concurrent user edits. Power-loss behavior also depends on filesystem
sync semantics. Unknown interrupted actions are never automatically repeated.

Provider requests are bounded and cancellable. Process time is limited to 120 seconds
and captured output to 64 KiB; excess output is drained/discarded. File reads are at
most 512 KiB, ranges 300 lines, and workspace snapshots at most 10,000 files/128 MiB.
Discovery prunes excluded directories and limits nesting to 32 path components.
Captured truncation is explicit; rehydration recovers captured bytes, not discarded
output. Pinned context overflow stops rather than silently dropping constraints.

Only selected environment variables needed by tools are inherited. API keys/account
tokens are not forwarded. Git inspection disables external diffs, textconv and
fsmonitor, restricts literal pathspecs, ignores submodules, and refuses configured
clean/smudge filters, including included Git configuration. Terminal controls
are stripped before display. Known environment secrets are redacted, including across
native stream fragments, but arbitrary secrets in source cannot be reliably detected.

The Codex bridge delegates enforcement to the official runtime and discloses its
permission boundaries. It does not duplicate tools or lower sandbox settings on
failure. Unsupported protocol versions stop with a diagnostic.

No private vulnerability inbox has been established. Do not post credentials or
exploit details in public issues; arrange a private reporting channel with the
repository maintainer before sharing sensitive material.

Native `run --auto-approve` (alias `--full-access`) is explicit session-wide consent
for supported patches and commands. It suppresses approval prompts, not policy
denies, path protections, stale checks, request caps or cancellation. It is persisted
with the session and remains active on resume; new sessions default to manual
approval. Each automatic approval records the exact candidate and patch diff. This
is not unrestricted shell/filesystem access and is rejected in Codex bridge mode.
Use it only with trusted repository code; commands have no OS sandbox.

Exact replacement proposals are not directly executable. The runtime requires the
observed file hash and exactly one matching snippet, constructs the full patch and
then runs ordinary validation/approval. No fuzzy replacement or model-computed hash
can bypass those checks. Saved interactive preferences exclude execution consent,
credentials and workspace paths.

Native project checks include approved pytest and existing npm test/build/lint/
typecheck scripts. npm offline mode prevents its package fetching, not networking
inside scripts. Lifecycle scripts and test plugins run trusted repository code;
user-level configuration may be read. See [supported checks](docs/project-checks.md).
