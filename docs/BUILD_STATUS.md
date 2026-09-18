# Build status — 2026-09-19

**Runnable experimental v0 core. Full live release acceptance is not yet established.**
The native API integrations are implemented and tested against local HTTP fixtures,
with provider-restricted live diagnostics maintained privately. A complete delegated
coding turn remains unverified; private native results are not published here. No performance claim, package publication, release upload, push, or hosted
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

## Live check rejection — 2026-09-19

The user ran the two-request launcher and supplied a screenshot of Claude HTTP 400.
Claude failed before OpenRouter was attempted. The old adapter discarded the error
body, so the underlying account/request cause remains unknown. This is a failed live
check, not a successful integration validation. Added bounded/redacted JSON error
messages and request IDs, without retry or fallback, and removed the Claude live
check's unwrap panic. Unit and local HTTP regression tests cover detail preservation,
secret removal, non-JSON/oversized responses and a single request with no deltas.
No new live request was made by the agent.
Validation: 40 offline tests, fmt/clippy, release build and the 228-package license
inventory passed after this fix. Live root cause still requires a new provider
response; the previous response body cannot be recovered from the screenshot.

## Official Jev route — 2026-09-19

The local live launcher and new home settings now default to direct TypeSafe Jev,
matching CLI defaults. Existing sessions retain their recorded route. Added
`python3 scripts/try_live.py jev-check` for one explicit, isolated Jev request without
Claude; `check` and `task` now pair Claude with TypeSafe by default. OpenRouter remains
an explicit `--jev-provider openrouter` choice. Key validation is provider-specific:
TypeSafe `apikey_` secrets are accepted; masked dashboard previews are rejected.
Four offline launcher regressions verify provider routing, isolated child credentials,
key formats and stopping before credential collection without budget consent.
No live TypeSafe request was executed here; user key is not installed or persisted.
Validation for the route change: four Python launcher tests and six Rust library
tests passed; formatting, all-target clippy and release build passed. The existing
full 40-test Rust suite last passed in the immediately preceding diagnostics change.

## Opus selection and visible stop reasons — 2026-09-19

New Claude sessions and the live launcher default to claude-opus-5 at user request;
`--model` explicitly overrides the launcher model for both check and task. Existing
sessions retain their model. Rechecked the official model catalog. No live Opus
request or successful Opus task has been verified. No comparative claim is made.
The conversation now shows no-progress and blocked reasons without opening the
inspector. Generation instructions include concrete read/search examples and require
proposed actions to match the visible plan; runtime permissions remain authoritative.
The unchanged no-progress guard still stops repeated planning without fresh evidence.
Validation: 41 offline Rust tests, five launcher tests, fmt, clippy, release build
and the 228-package license inventory passed. Provider-restricted live traces remain
private and were not added to git or exported.

## Remembered launcher credentials — 2026-09-19

The macOS source launcher now stores newly entered provider keys in the login
Keychain and reuses them. Explicit environment keys take precedence without being
copied to Keychain. No secrets are passed through argv or written to repository,
session, or plaintext configuration files. Direct Security framework calls use the
legacy generic-password APIs verified against local Xcode SDK headers. Records are
namespaced by service `s1code.credentials.v1` and provider. Access denial fails closed;
`--no-keychain` is explicit ephemeral mode and `--forget-keys` deletes only these
records without requests. Linux remains environment/ephemeral input. This is a
source-launcher feature; the standalone Rust binary still reads environment keys.

Eight offline launcher tests passed, including reuse, one-time storage, denied
access and deletion scope. A uniquely named dummy credential passed real macOS
Keychain create/read/update/delete; the dummy record was removed and absence checked.
No real user credential was read, saved or submitted during implementation. No Rust
code changed; the preceding Rust verification remains applicable.

## Claude action-schema compatibility follow-up — 2026-09-19

Simplified only the Claude provider wire schema to one action object with nullable
unused fields; canonical domain actions and policy are unchanged. Strict conversion
rejects unknown/cross-action arguments and preserves nested null patch hashes.
This is an experimental compatibility fix, not a confirmed live root-cause claim.
No-progress protection remains enabled. No new paid request was made. Provider
outputs/metrics remain private. See ADR 005 for scope and validation limits.
Validation: 42 offline Rust tests, eight launcher tests, fmt/clippy, release build
and 228-package license inventory passed. The local HTTP native patch/verification
integration also passed after its fixtures were updated to include nullable wire
arguments. A new bounded live task requires explicit spend consent; not run yet.

## Native action progress and explicit budget continuation — 2026-09-19

- Claude wire arguments now live in separate named nullable payload objects, not
  shared nullable fields. Cross-action payloads/fields are rejected. Canonical
  runtime actions, hash validation and approvals remain unchanged.
- Low-confidence selection prefers an untried policy-allowed read/search/rehydration
  candidate before requesting another plan. This deterministic evidence fallback
  is separately labeled; it does not grant write/process permissions or claim
  confidence is correctness. Prior tool executions are not automatically replayed.
- Planning fingerprints include the proposal contract version so schema changes
  can be retried against existing evidence without resetting request counters.
- Native resume supports explicitly changing total request/generation caps. Existing
  usage is preserved, invalid caps are rejected and budget changes are journaled.
- Authorized live diagnostics were conducted in a disposable private workspace;
  provider-restricted traces and outcomes remain outside the repository. No live
  performance figures or comparative claims are published here.
- Regression tests cover named payload conversion, cross-action rejection,
  evidence-before-replanning, repeat avoidance and persisted budget changes through
  CLI approval/resume. See ADR 005. Existing stored sessions remain usable.
Final local verification: 44 offline Rust tests, eight launcher tests, formatting,
all-target clippy, release build and 228-package license inventory passed. The CLI
headless approval/resume smoke passed with journaled budget updates and preserved
counters. No provider-restricted trace or live measurement was staged for commit.
