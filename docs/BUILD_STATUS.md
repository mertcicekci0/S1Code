# Build status — 2026-09-19

**Runnable experimental v0 core. Full live release acceptance is not yet established.**
The native API integrations are implemented and tested against local HTTP fixtures,
but billed OpenAI/Claude/Jev inference and a complete delegated coding turn have not been
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

- 37 offline tests passed: real coding loop, three approvals, restart, traversal,
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
python3 scripts/home_smoke.py
python3 scripts/headless_smoke.py
cargo test --test live codex_local_protocol -- --ignored
cargo run --locked -- eval --suite fixtures/core
cargo run --locked -- eval --suite fixtures/heldout --output eval-results/heldout
```

Explicit billed Jev contract check (not run here): configure TYPESAFE_API_KEY and
S1CODE_LIVE_BUDGET_REQUESTS, then `cargo test --test live jev_live_contract -- --ignored`.
Use the documented `s1code eval --live` command for actual coding trials with both
credentials and execution/request-budget consent. Keep results private.

## 0.2.0 follow-up — implemented and locally verified

- Product/package/executable renamed to S1Code / `s1code`; legacy v1 sessions
  are reused without destructive migration. The workspace lock namespace remains
  shared with old executables. All 31 offline tests passed after the rename.
- Task-entry home screen opens with no inference, shows provider/auth/decision
  ownership, accepts Unicode editing/paste and slash commands, handles missing keys,
  lists/resumes sessions and returns after each task. New prompts start independent
  tasks; there is no implied cross-task conversation memory.
- Native Claude streaming Messages adapter, structured proposals, provider-persisted
  resume and matched evaluator settings. Four new provider tests cover fragmented
  streams/usage/hidden-content handling, invalid/partial/refused responses, in-flight
  cancellation and a complete local-HTTP patch/real-test cycle with three approvals.
- Explicit official Claude Code terminal handoff, separate from native or Codex
  bridge ownership. The handoff process test uses a labeled local stub and confirms
  no bypass/task arguments and no unrelated provider keys. Actual CLI version
  2.1.266 was observed; authenticated Claude Code coding remains unverified.
- Home/terminal rendering tests and PTY workflow checks passed; 37 offline tests,
  formatting, clippy with warnings denied, release build and 228 dependency license
  checks passed. No new dependencies were added. Four live tests remain opt-in;
  no billed Claude/OpenAI/Jev or managed coding task was run in this follow-up.

## Conversation follow-up — 2026-09-19

- Conversation is the default task view; wrapped replies are retained, protocol
  events and four technical tabs move behind F2. Approvals and patch review remain
  visible. Native tasks still use bounded runs, not an ongoing chat loop.
- Codex interactive responses accept another message in the same thread and App
  Server process. Every submitted message is a separately counted delegation.
  Closing and resuming observes the saved turn without resubmitting it. Responses
  without verification are awaiting_input, never reported as verified completion.
- Corrected stream-tail labeling so command/file output cannot become assistant
  dialogue. Full diagnostic events remain recorded. Wall time includes user input
  and approval waits; upstream inference latency/usage remain unknown.
- Added OFFLINE protocol/PTY smoke test for two turns, runtime/thread reuse and
  resume without duplicate execution. Added default-view/narrow-layout regression
  test. Ratatui's pinned rendered-line-info feature computes wrapped scroll limits.
- Hidden-input live launcher prepares builds without keys and requests a concrete
  request budget before collecting credentials. No keys from chat were stored or
  used. Live checks, paid coding runs and comparative efficiency remain unverified.
- Verification for this follow-up: 38 offline Rust tests, fmt, clippy (`-D warnings`),
  release build and 228-package license inventory passed. All four PTY/headless smoke
  scripts passed, including `python3 scripts/conversation_smoke.py`. Launcher help
  and key-isolated child environments were checked without provider requests.
