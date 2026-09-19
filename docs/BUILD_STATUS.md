# Build status — 0.3.0-rc.1

Experimental source release candidate, prepared 2026-09-19. Native coding and
recoverable context are implemented. This is not a stable general-purpose release,
and no cost, speed or task-quality superiority claim is established.

## Implemented

- Independent Apache-2.0 Rust library and binary, centralized S1Code branding,
  Rust 1.94.0, Cargo.lock and backward-compatible versioned session storage.
- Native loop: bounded discovery and literal evidence candidates, generator
  proposals, deterministic filtering, rules/Jev/constrained-generative selection,
  stale-state checks, exact approvals, execution, persistence and verification.
- Streaming OpenAI Responses and Anthropic Messages adapters, strict structured
  action validation, cancellation and provider-native usage. Claude defaults to
  Opus 5; model IDs are configurable. No silent provider switch.
- Direct TypeSafe Jev Choice/Noul/Score integration; optional explicit OpenRouter
  gateway; version checks, question batching, exact cache, conservative token
  estimates, bounded retries and experimental confidence/retention policies.
- Bounded reads/search, git status/diff, hash-bound whole-file patches with recovery,
  approved Python unittest and offline Cargo checks, writer lock, process-group
  cancellation, path/symlink/ignored-file protection and explicit exclusions.
- Append-only events, atomic checkpoints, content-addressed artifacts, canonical
  call/result linkage, dependency closure, reversible eviction and exact rehydration.
  Unknown interrupted actions are not automatically rerun.
- Context compaction reads each active artifact once per pass; exact serialized
  size accounting handles Unicode/escaping. Whole eviction plans commit only if
  they fit. Tiny results stay active when a placeholder would enlarge them.
- Jev retention batches send only the evidence being scored. Claude uses stable
  task/evidence blocks before dynamic metadata and a five-minute cache breakpoint.
  Actual cache reads/writes are recorded; cache hits or savings are not assumed.
- Conversation-first terminal with streaming, task input, approvals and optional
  activity/decision/context/diff inspector; resize and cancellation. Same engine
  provides JSONL headless output, durable approval/resume and sanitized replay.
- Official local Codex App Server bridge with managed ChatGPT login, checked
  permissions, events, approvals, interruption and saved-thread continuation.
  Delegations are not counted as native generation calls. Official Claude Code
  terminal handoff is separate; no subscription-token reuse is implemented.
- macOS source launcher reuses provider keys from Keychain. Standalone Rust CLI
  still reads environment credentials; Linux launcher input is temporary.
- Offline demo with real patch/test execution, context mechanics probe, independent
  fixture checks, matched-policy evaluator, explicit live spend caps, private traces,
  source/history publication scan and third-party license inventory.

## Verification recorded locally

- 49 offline Rust tests and eight Python launcher tests passed. Coverage includes
  fragmented streams, invalid decisions, retries, stale candidates, policy bypass,
  traversal/symlinks, concurrent edits, patch recovery, pinned overflow, exact
  rehydration, dependency closure, cancellation, restart, duplicate bridge requests,
  redaction, transactional compaction and stable-prefix preservation.
- Formatting, all-target clippy with warnings denied and release build passed.
- Real PTY parser demo completed with three approvals, resize at 50/110 columns,
  simulation label and terminal restoration. Headless approval/resume/export passed.
  Offline Codex protocol fixture handled two turns and resume without resubmission.
- Five development fixtures and one held-out fixture fail their starting protected
  checks and pass their reference patches. This validates the evaluator, not agent
  performance or success rates.
- The offline context probe completed all 15 trials, checking actual artifact
  storage, eviction budgets and exact rehydration. No provider call or token-cost
  claim is part of that probe.
- Host license inventory: 228 dependencies. The local advisory scan reported no
  findings using RustSec revision documented in dependency-audit.md.
- Provider-restricted live diagnostics, traces and outcomes remain private. The
  explicit native context-pressure check uses real APIs and bounded fixture actions;
  public source includes the test, not its outputs or service measurements.

## Limitations and outstanding validation

- A full live delegated Codex coding task remains unverified. Local protocol and
  managed account checks are not a substitute. Supported local CLI: 0.153.3;
  unsupported versions fail with setup guidance.
- Live task-success/cost/latency comparisons across rules, Jev and constrained
  generation have not established an advantage. Eviction quality, cache reuse and
  invalidation costs require repeated matched trials; keep Jev results private.
- Linux is a target with CI configuration; local verification was on macOS ARM64.
  Hosted CI results must be checked on GitHub after pushing. Windows is unsupported.
- Native commands execute trusted repository code without an OS filesystem/network
  sandbox. Read allowlists and worktrees do not provide OS isolation. Native path
  exclusions cannot control upstream Codex internals.
- Native tasks are bounded runs, not a full multi-turn coding chat. Only Python
  unittest and offline Cargo test/check execute. No package installation, general
  shell, deletion/rename/binary patch, swarms, daemon or local learned model.
- Patches require existing parent directories. Multi-file updates are recoverable,
  not atomic as a set. Rehydration restores captured bytes, which may be historical
  or originally truncated; it does not guarantee the model requests useful evidence.
- Retention batches still repeat the task/rubric. Large pinned context fails
  explicitly; byte budgets and Jev token estimates are not tokenizer measurements.
- Keys pasted into external conversations are outside local secret-storage control.
  Release scanning is best effort; no telemetry, private session export or bundled
  provider credentials are included in the source release.

## Reproduce

```sh
scripts/check.sh
python3 scripts/release_check.py
python3 scripts/headless_smoke.py
python3 scripts/terminal_smoke.py
python3 scripts/conversation_smoke.py
cargo run --locked --release --example context_probe
./target/release/s1code eval --suite fixtures/core
./target/release/s1code eval --suite fixtures/heldout --output eval-results/heldout
```

Live checks require configured keys and a separately approved request budget.
See providers.md, evaluation.md and demo.md. No published service benchmark,
training dataset or model imitation is part of this release.
