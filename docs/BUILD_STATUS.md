# Build status — 0.3.0-rc.5

Experimental source release candidate, prepared 2026-09-19. Native coding and
recoverable context are implemented. This is not a stable general-purpose release,
and no cost, speed or task-quality superiority claim is established.

## Implemented

- Installed native `auth set/remove PROVIDER` and home `/auth PROVIDER` manage
  API keys through macOS Keychain, with hidden input and no model request. Existing
  saved credentials remain compatible; OpenAI now supports saved keys too. Linux
  reports its environment/secret-manager requirement. Offline validation covers
  provider mismatches and hidden-input rejection, without writing user credentials.

- Recognized zero-test summaries no longer satisfy completion. Actual empty
  unittest execution and mixed Cargo/Node/Python summaries have regression coverage;
  18 native integration tests pass. Compiler checks remain separately supported.

- Workspace-scoped `resume` defaults to the latest saved task; unique ID prefixes
  and task previews make existing sessions findable. Native interactive resume
  restores recent activity for inspection without replaying tools or approvals.
  Checkpoint identity is revalidated after taking the workspace lock. Library,
  native integration and headless approval/resume tests pass locally.

- Task conversations display the final completion explanation and process exit
  results. Ctrl+O opens the inspector without function keys; Esc returns to chat.
  Follow-up `/help`, `/activity`, `/decisions`, `/context` and `/diff` are local
  commands; unknown slash commands never consume inference. UI regressions and
  the offline delegated-conversation PTY test cover this boundary.

- Command hardening: unittest discovery checks directory existence, exclusions,
  ignore rules and symlinks before approval and again before spawning. Duplicate
  start-directory flags are denied. Unsupported-command feedback reaches planning.
  Policy version 5 invalidates earlier approvals. The 16 native integration tests
  and two command-policy tests pass locally; no provider requests were used.

- Independent Apache-2.0 Rust library and binary, centralized S1Code branding,
  Rust 1.94.0, Cargo.lock and backward-compatible versioned session storage.
- Native loop: bounded discovery and literal evidence candidates, generator
  proposals, deterministic filtering, rules/Jev/constrained-generative selection,
  stale-state checks, exact approvals, execution, persistence and verification.
- Search-derived candidates carry their own bounded, redacted matching line into
  decision state. The terminal inspector shows the selected action and its evidence;
  a matched misleading-result fixture exercises the ambiguity without forcing Jev
  into deterministic steps.
- A harder same-model demo prepares byte-identical Claude Code and S1Code trees with
  seven plausible payment-code locations. The decision frontier stays capped and
  retains an explicit generator escape route.
- Streaming OpenAI Responses and Anthropic Messages adapters, strict structured
  action validation, cancellation and provider-native usage. Claude defaults to
  Opus 5; model IDs are configurable. No silent provider switch.
- Direct TypeSafe Jev Choice/Noul/Score integration; optional explicit OpenRouter
  gateway; version checks, question batching, exact cache, conservative token
  estimates, bounded retries and experimental confidence/retention policies.
- Bounded reads/search, git status/diff, hash-bound whole-file patches with recovery,
  approved Python unittest (including bounded `discover -s <relative-dir>`), Node.js tests and offline Cargo checks, writer lock, process-group
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
  now also reads saved macOS Keychain credentials directly; Linux launcher input is temporary.
- Offline demo with real patch/test execution, context mechanics probe, independent
  fixture checks, matched-policy evaluator, explicit live spend caps, private traces,
  source/history publication scan and third-party license inventory.

## Verification recorded locally

- 86 offline Rust tests and 12 Python launcher/comparison tests passed. Coverage includes
  fragmented streams, invalid decisions, retries, stale candidates, policy bypass,
  traversal/symlinks, concurrent edits, patch recovery, pinned overflow, exact
  rehydration, dependency closure, cancellation, restart, duplicate bridge requests,
  redaction, transactional compaction, stable-prefix preservation and explicit
  automatic approval with real patch/test execution and preserved policy denies.
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
- Linux and macOS hosted CI passed for commit 984dd96; newer changes require their
  own hosted checks after pushing. Local verification is on macOS ARM64. Windows is unsupported.
- Native commands execute trusted repository code without an OS filesystem/network
  sandbox. Read allowlists and worktrees do not provide OS isolation. Native path
  exclusions cannot control upstream Codex internals.
- Native conversations support bounded follow-ups with cumulative metrics. Only Python
  unittest, exact node --test and offline Cargo test/check execute. No package installation, general
  shell, deletion/rename/binary patch, swarms, daemon or local learned model.
- Approved patches can create allowed parent directories. Multi-file updates are recoverable,
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

## Snake comparison preparation

A shared one-prompt Snake task and two identical starter repositories can be created
with `python3 scripts/snake_compare.py prepare`. Official Claude Code uses managed
auth; native S1Code reuses saved API credentials. Both receive the same prompt and
requested model; tool/auth differences are recorded rather than hidden. No trial is
automatically started. A private local preview serves each game on its own origin.
Exact `node --test` is now an approval-required native command; extra Node arguments,
script paths and npm remain denied. This executes trusted repository code without
an OS sandbox. Policy version 2 invalidates old pending approvals; start a fresh
task rather than reusing an approval from the earlier command policy. The generation
contract was updated to advertise the supported command.

Explicit native `--auto-approve` / `--full-access` now grants session-wide consent
for supported patches/tests. Defaults and old stored sessions remain manual. It is
visible in the terminal, recorded per candidate, persists on resume, and is rejected
for the Codex bridge. Revalidation, denies and cancellation stay enforced. The
offline real-tool regression completes the parser task without approval prompts and
checks persisted consent; denied/tampered/stale candidates still fail. No paid Snake
trial has been started by the preparation or verification scripts.

Native credential usability: direct CLI reads existing macOS Keychain entries for
Claude, TypeSafe and OpenRouter, with explicit environment override and no child
environment mutation. Credential lookup errors never include captured key output.

Interactive home supports explicit `/permissions full-access` for native tasks,
shows auto-approval state and resets consent on provider change. Users can paste
tasks directly into the home input without a shell wrapper.

Response recovery and terminal readability: Claude terminal stop reasons are now
recorded distinctly, including usage for complete truncated responses. A max_tokens
response permits one smaller-action retry within existing generation/request caps;
refusals do not retry; rc.3 adds separate bounded transport recovery. Partial actions never run.
Long tasks are folded in conversation while exact text remains in Activity;
paste preserves newlines, tool progress/recovery are readable and empty assistant
sections are hidden. The previously failed live Snake trace did not retain its stop
reason, so its root cause cannot be established retrospectively. At that milestone live coding acceptance was still pending. Subsequent private
acceptance records are retained outside the published source.

Native execution milestone: Claude client-tool proposals replace the nullable
text-output envelope on outgoing requests. Fragmented tool JSON is collected and
validated only after a complete terminal event; runtime still owns all execution.
Output limits are configurable and Opus/Sonnet 5 use explicit medium effort by
default. Reported reasoning usage is recorded as a subset of output. Empty workspace
searches are skipped, and rejected candidate diagnostics reach subsequent planning.
Provider integration tests (17), library tests (17) and all-target clippy passed.
The local HTTP fixture performs a real patch and verification with streamed tool
calls. These protocol tests are offline; they must not be reported as live model results.

Exact replacement proposals now materialize into a complete, hash-bound patch before
selection. Ambiguous, missing, unchanged, overlapping and stale snippets fail without
mutation; approvals and rollback use the existing full-patch path. Interactive home
remembers provider/model/decision preferences (not workspace, credentials or consent),
preserves multiline input, and resets auto-approval on either provider-switch route.
Keychain presence is cached for rendering and permission waits have a 15-second bound.

Final native hardening: context views now replace full patch payloads with file/hash
metadata, so eviction cannot accidentally retain a second copy of code in action
arguments. Canonical actions and exact artifact bytes remain recoverable. A
regression covers this boundary. The default generation sub-budget is 24 while the
combined provider request cap remains 24; this avoids premature generation-only
stops without increasing the total request cap. All 74 Rust and 12 Python checks,
formatting, clippy, release build and publication scan passed locally. Private live
acceptance was performed; provider-restricted records are not included in Git.

Workspace ownership is explicitly unlocked when a store is dropped, rather than
waiting for every inherited descriptor to close. A duplicate-descriptor regression
covers immediate resume and continued exclusion while the resumed store is active.
This addresses an intermittent Linux CI failure during concurrent process spawning.
CI now checks out full history for the publication scanner.

## Demo hardening

- Native follow-up input in the task view; exact prior user requests remain pinned,
  verification and prior proposals reset, all usage remains cumulative. Headless
  `resume --message` keeps existing caps unless explicitly changed.
- Focused Jev relevance snapshots, actual serialized request/state byte counters,
  adaptive retention batches, feasibility checks before paid classification, and
  early stop once enough evidence can be evicted. Bytes are not token or cost estimates.
- Small independent outputs avoid retention requests; call/dependency groups stay
  together. Patch dependencies use matching file evidence, not arbitrary nearby tools.
- Nested file creation with a backward-compatible recovery journal; conflicting
  file/parent paths, symlinks and ignored targets fail before mutation. Recovery
  preserves user files in created directories. Policy version 3 invalidates old approvals.
- `/eviction` exposes retention policy in the home screen and saved preferences.
- New behavior has offline regression coverage; no new paid comparative evaluation
  was performed. The earlier private live coding acceptance remains a separate check.

## rc.3 conversation and response recovery

Exact greetings return locally without file discovery, tests or provider requests.
Informational replies use `answer` / `awaiting_input`; they cannot mark an unverified
coding task completed. Claude HTTP/SSE diagnostics now retain sanitized type and
request ID. Transient generation retries are engine-owned, bounded by two retries
and existing request caps, cancelable, and never repeat previously completed tools.
See ADR 009. No additional paid provider calls were made for this correction.

rc.3 local verification: all 82 offline Rust tests, 12 Python checks, formatting,
all-target clippy with warnings denied, release build and four terminal/headless
smoke checks passed. The real native CLI answered a greeting with a zero request
cap and no tools. Recovery fixtures exercise transient exhaustion, permanent
failure, cancellation and retry-budget exhaustion after actual patch/test execution.
No new live provider inference was used; old generic stream errors cannot reveal
provider details that were never persisted.

## rc.4 model selection guard

The task-entry screen accepts `/model opus` and `/model sonnet` as complete Claude
model aliases. A model ID ending in `-` is rejected locally, preventing the known
mistyped `claude-opus-` value from consuming a failed provider request.
