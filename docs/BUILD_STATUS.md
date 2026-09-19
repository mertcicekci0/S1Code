# Build status — 0.3.0-rc.8

Experimental source release candidate, prepared 2026-09-19. This is a bounded coding
agent for trusted repositories, not a stable unrestricted automation product.
Cost, speed and task-quality advantages have not been established.

## Implemented

- Apache-2.0 Rust library/binary, Rust 1.94.0, Cargo.lock, centralized S1Code names
  and independently versioned session storage. No reference-project code was copied.
- Native runtime owns candidate construction, deterministic allow/ask/deny policy,
  bounded selection, execution and verification. A single concrete action bypasses
  decision inference. Rules, Jev and constrained generation are distinct policies.
- Streaming OpenAI Responses and Claude Messages adapters validate complete actions,
  record reported usage, reject partial actions and bound recovery attempts. Provider
  errors never cause silent transmission to another provider.
- Official TypeSafe Jev and explicitly chosen OpenRouter gateway: typed questions,
  answer validation, two request limits, cancellation, bounded retries and exact
  caching. Focused relevance evidence and independently batched retention questions.
- Bounded repository listing/search/read, Git inspection, exact replacements and
  recoverable multi-file patches. Python unittest, Node test, pytest, approved npm scripts and offline Cargo
  test/check commands execute only after manual or explicit session-wide consent.
- Directory validation covers explicit test-discovery paths before approval and
  again before spawning. Exclusions, ignored paths, symlinks and duplicate start
  directories are rejected. Invalid actions provide actionable planning feedback.
- Completion needs a current successful verification and a completion proposal.
  Recognized zero-test summaries cannot satisfy it; compilation checks do not claim
  to run tests. Passing checks do not prove full task correctness.
- Durable journals, atomic checkpoints, content-addressed artifacts, one workspace
  writer, crash recovery, process-group cancellation and safe session resume.
- Reversible eviction with budget hysteresis, pinned evidence, dependency closure
  and exact rehydration without repeating commands. Compaction is transactional;
  pinned overflow fails explicitly. Cache reads/writes remain reported observations.
- Conversation-first terminal with streaming, readable outcomes, exact approvals
  and an optional inspector. Ctrl+O/F2, local slash commands and narrow layouts.
  Headless JSONL uses the same engine. Native follow-ups retain prior constraints.
- Workspace-scoped latest-session resume, unique ID prefixes, task previews and
  restored recent native activity. Display restoration cannot reactivate approvals
  or rerun tools. Checkpoint identity is checked after taking the workspace lock.
- Official local Codex App Server bridge with managed login, checked permissions,
  events, approvals, interruption and saved-thread continuation. Its hidden turns
  and context are upstream-owned. Official Claude Code terminal handoff is separate.
- Standalone macOS hidden API-key setup via Keychain; existing entries are reused.
  Environment overrides remain explicit. No provider call during key storage.
- Official Claude account commands with identity-filtered status and an optional
  MCP stdio companion for batched Jev evidence ranking. Required request cap,
  cancellation, exact caching, bounded inputs and no local execution or host-context
  replacement. OAuth account flow stays in the official client.
- Existing npm test/build/lint/typecheck scripts and installed pytest are supported
  with explicit execution consent; scripts and plugins remain trusted code.
- Offline demos with real tools, context-pressure probe, protected fixture checks,
  matched-policy evaluation tooling and source/history publication scanning.
- Reproducible source archives from clean commits, checksums and license inventory.
  CI runs source checks on macOS/Linux and a separate dependency advisory check.

## Verification

rc.8 local checks passed: 110 offline Rust tests, 19 Python tests, formatting,
all-target clippy, six headless/PTY/credential/MCP smoke scripts, release build, six evaluator fixtures and the 15-trial context probe. The
advisory audit reported no findings. actionlint 1.7.12 accepted both workflows
(shellcheck and pyflakes integrations disabled). Binary release preparation adds
two native target builds, installed-archive smoke checks and checksum/manifest
validation. Hosted rc.8 results and publication are pending at this commit.


Baseline macOS ARM64 validation before the rc.8 changes passed:

- 107 offline Rust tests, 12 Python tests, formatting and all-target clippy with
  warnings denied; optimized release build.
- Five headless/PTY checks plus an MCP binary protocol check: real parser failure/patch/verification with three exact
  approvals, resize, home navigation, same-thread conversations, safe resume,
  local inspection commands, redacted export and terminal restoration.
- Hidden credential-input mismatch rejection and restored echo, without changing
  user Keychain entries or sending inference requests.
- Five development fixtures and one held-out fixture fail their starting protected
  checks and pass their reference patches. This validates the evaluator, not models.
- The context probe completed 15 offline trials of real artifact storage, eviction
  budgets and exact rehydration. No token savings or model quality is inferred.
- License inventory for 234 host dependencies and an advisory scan with zero findings.
  Generated notice files remain under target/notices for binary redistribution.

Hosted macOS/Linux checks and the dependency-audit job passed for source commit
`02219ee` ([run](https://github.com/mertcicekci0/S1Code/actions/runs/35454722733)).
CI pins the checkout action to a reviewed release commit, disables persisted
checkout credentials and pins the Ubuntu runner label.

The source/history scan has no pattern findings. This is not proof that arbitrary
sensitive content is absent. The dated dependency audit is documented separately
in [dependency-audit.md](dependency-audit.md).

## Remaining limits and unverified paths

- Provider acceptance and live validation records remain private; public claims
  here do not establish comparative performance. Mock contracts, demonstrations
  and fixture reference patches do not establish live task success or efficiency.
- Full live delegated Codex coding remains unverified. The supported local CLI is
  0.153.3; other versions stop with compatibility guidance. No internal generation
  call count, context ownership or subscription price is inferred from delegations.
- Actual macOS credential writes depend on OS Keychain authorization; the offline
  suite does not change user credentials. Linux uses environment keys/secret managers.
- Native commands execute trusted repository code without OS filesystem/network
  isolation. Process groups cannot contain deliberately detached processes. The
  file-tool allowlist cannot restrict what approved repository code accesses.
- Claude Code companion tool discovery and wire behavior are tested; subscription
  login completion requires the user's official interactive account flow. MCP ranking
  is host-invoked advice, not automatic compaction or a controlled native loop. Its
  process budget resets on restart and is not a monetary or daily cap.
- No arbitrary shell, package installation, binary/delete/rename patches, daemon,
  swarms, local learned policy, or automatic commit/push. Auto-approval applies only
  to supported native actions. It is not unrestricted access.
- Multi-file patches are recoverable, not atomic as a group. Hostile same-user
  filesystem races and power-loss behavior beyond filesystem sync are not covered.
- Rehydrated bytes may be historical or originally truncated. It does not guarantee
  that a model requests the right evidence. Jev budgets use conservative estimates,
  not a supported tokenizer. Large pinned context stops explicitly.
- Windows is unsupported. Local checks run on macOS ARM64; hosted checks must be
  verified against the exact pushed commit before claiming this release passed CI.
- No registry package or hosted binary release has been published by this workflow.
  Naming availability and trademark clearance remain unestablished.

## Reproduce

```sh
scripts/check.sh
scripts/audit.sh
cargo run --locked --release --example context_probe
./target/release/s1code eval --suite fixtures/core
./target/release/s1code eval --suite fixtures/heldout --output eval-results/heldout
python3 scripts/source_archive.py
```

The source packager requires a clean committed tree. Live checks need separate
credentials, disposable fixtures and explicit spend limits. See [releasing](releasing.md),
[providers](providers.md), [evaluation](evaluation.md) and [demo](demo.md).
