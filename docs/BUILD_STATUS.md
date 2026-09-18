# Build status — 2026-09-19

**Runnable experimental v0 core. Full live release acceptance is not yet established.**
The native API integrations are implemented and tested against local HTTP fixtures,
but billed OpenAI/Jev inference and a complete delegated coding turn have not been
verified. No performance claim, package publication, release upload, push, or hosted
CI run occurred.

## Implemented

- Independent Apache-2.0 Rust package, library/CLI separation, pinned Rust 1.94.0,
  Cargo.lock, source installation and centralized brand/storage version constants.
  The workspace was empty. The separate sibling project was left untouched.
- Native loop owns concrete candidates, deterministic input filtering, selection,
  workspace/policy revalidation, exact approvals, tools, evidence and verification.
  Rules are deterministic; Jev is a specialist adapter; constrained generative
  selection is an explicit comparison policy. Escapes and no-progress/request caps
  are implemented. Generation includes planning/diagnosis/search and is counted.
- Streaming Responses API-key adapter with structured actions, fragmented SSE,
  typed validation, usage, cancellation, refusal/error handling and no cross-provider
  fallback. Default configurable model: gpt-4.1-2025-04-14.
- Real Jev HTTP adapter: Choice/Noul/Score, answer/range/distribution validation,
  pinned version, independent-question batches, exact cache, two conservative token
  estimates, bounded retry/jitter/Retry-After and explicit fallback policy.
- Bounded discovery/search/read, hash-validated whole-file patches with recovery,
  approved offline Cargo/Python checks, controlled process groups, Git status/diff,
  path/symlink/ignored-file protection and user exclusions. No automatic commits.
- Private versioned sessions, append-only events, atomic checkpoints, hashed
  artifacts, call/result grouping, protected dependencies, reversible eviction,
  exact rehydration, conservative compaction fallback and explicit pinned overflow.
  Interrupted/unknown actions are not blindly repeated.
- Official Codex stdio bridge with managed ChatGPT login/status/logout, checked
  CLI/protocol boundary, granular user approvals, read-only/network-disabled defaults,
  thread/turn events, cancellation, saved-turn observation/reattachment and explicit
  continuation. Upstream tools are never executed locally. Internal usage is unknown.
- Four-view terminal with streaming, expandable activity, candidate/decision details,
  context, diff, approvals, resize and cancellation; JSONL headless mode uses the
  same native engine. Export strips known secrets/paths; every replay is labeled.
- Version 0.1.1 adds readable approval cards, activity/decision summaries, colored
  diffs, opt-in raw event inspection (`j`), human final summaries, and an honest
  no-model offline header. Explicit OpenRouter Jev routing uses its alpha Decisions
  API, a pinned serving build and OPENROUTER_API_KEY.
- Offline parser demo with real failing/passing tests and patch approvals; real
  context-pressure demo without any model; five development and one held-out fixture;
  private evaluator with explicit live consent, matched settings and randomized order.
- README/quickstart, architecture/ADRs, provider setup, security/privacy,
  contribution/license/provenance, evaluation/demo docs and local/CI check tooling.

## Observed verification

- 30 offline tests passed: real coding loop, three approvals, restart, traversal,
  symlinks/ignored files, stale and tampered candidates, whole-patch prevalidation,
  recovery conflicts, fragmented/partial streams, permanent provider errors,
  retry exhaustion, exact cache invalidation, both Jev budgets, pinned overflow,
  exact rehydration/integrity, dependency closure, cancellation of descendants,
  duplicate/changed bridge requests, protocol framing, redaction and private export,
  journal/checkpoint divergence, partial-tail rejection, Git filter rejection,
  transitive eviction/rehydration, restored file permissions, mixed SSE framing,
  stale verification after external edits, and execution of an already selected
  action at the provider request cap. Default live tests skip.
  Added readable terminal rendering checks at 50/110 columns, streaming message
  extraction, and OpenRouter contract/build-drift/missing-field tests.
- Real PTY smoke test completed the parser task with three approvals, resized between
  50 and 110 columns, kept the simulation label visible, and restored the terminal.
- Headless demo completed across approval/resume processes; real verification exit 0;
  export/replay labels and personal-path removal checked. Generation calls: zero,
  because this is an explicitly simulated driver.
- Core (five) and held-out (one) fixtures fail their initial protected checks and pass
  after reference patches. These are **evaluator validation**, not agent success rates.
- Real context demo captured five reads, evicted two artifacts and recovered exact
  stored bytes without rerunning a command. No Jev call or scripted answer involved.
- Installed official codex-cli 0.153.3 passed initialize/account-read and disposable
  ephemeral thread creation with checked permissions. Granular policy requires the
  experimental capability in this executable; that opt-in is explicit. No login,
  logout, account setting mutation or inference was performed by those checks.
- Formatting, clippy with warnings denied, tests and optimized release build passed
  locally during development. Final check command: `scripts/check.sh`.
- Host dependency license/notice check passed (228 packages on aarch64-apple-darwin).
  The advisory audit after the terminal-library upgrade reported zero vulnerabilities
  and zero warnings; see dependency-audit.md for tool/database versions and findings.

## Unverified or limited

- OpenRouter gateway inference remains unverified; no paid request was made.
  Managed ChatGPT account metadata was read successfully, which is not a full
  delegated coding validation.
- OpenAI and TypeSafe keys were absent. Live adapter compatibility, useful generated
  patches, actual Jev decisions/eviction quality, full managed login/logout completion
  and a delegated coding task remain unverified. Explicit live spend consent is needed.
  A local contract test is not a live provider test.
- Linux is a target with CI configuration, not an observed test result in this session.
  Windows is unsupported. Hosted CI has not run here.
- Native process execution has no OS filesystem/network sandbox. Approved repository
  code is trusted; hostile same-user filesystem races/daemonization are outside the
  guarantee. Native excluded paths cannot be enforced inside the Codex runtime.
- Only Python unittest and offline Cargo test/check are executable in native v0.
  No installer, general shell, deletion/rename/binary patch, nested instruction
  hierarchy, remote tool/plugin system, daemon, swarm or local learned model.
- Patches require existing parent directories. Multi-file writes are recoverable,
  not atomic as a set. Truncation means exact rehydration of captured bytes only.
- Token budgets use deliberately conservative estimates. Eviction confidence/retention
  thresholds are experimental. No dollar accounting is fabricated from unknown rates.
- Headless bridge approval is denied; interactive terminal is required. Upstream
  verification is explicitly labeled and inferred from completed test/check commands,
  not independent proof. Native request caps cannot meter hidden upstream calls.
- The evaluator is not an adversarial OS sandbox. It keeps protected checks outside
  native tool roots, but same-user malicious test code could attack the controller.
- Session records are private but unencrypted. Unknown source secrets can survive
  redaction. Inspect exports before sharing. Jev exports remain blocked pending
  documented clearance, and evaluator results are gitignored/private.

## Reproduce

```sh
scripts/check.sh
cargo build --locked
python3 scripts/terminal_smoke.py
python3 scripts/headless_smoke.py
cargo test --test live codex_local_protocol -- --ignored
cargo run --locked -- eval --suite fixtures/core
cargo run --locked -- eval --suite fixtures/heldout --output eval-results/heldout
```

Explicit billed Jev contract check (not run here): configure TYPESAFE_API_KEY and
NERVE_LIVE_BUDGET_REQUESTS, then `cargo test --test live jev_live_contract -- --ignored`.
Use the documented `nerve eval --live` command for actual coding trials with both
credentials and execution/request-budget consent. Keep results private.
