# Changelog

## 0.3.0-rc.6 — Release hardening

- Test-discovery paths obey exclusions, ignore rules and symlink restrictions,
  including a check immediately before execution. Policy version 5 invalidates
  earlier pending approvals. Rejected commands explain supported alternatives.
- Recognized zero-test runs cannot verify a coding task, even with exit code zero.
- Final explanations and command outcomes appear in the conversation. Ctrl+O
  opens the inspector on keyboards without accessible function keys. Task-local
  slash commands inspect history, decisions, context and diffs without inference.
- `resume` defaults to the latest task in the current workspace and accepts unique
  ID prefixes. Saved activity returns without replaying tools or old approvals.
- `auth set/remove PROVIDER` and home `/auth PROVIDER` manage native API keys through
  macOS Keychain with hidden input. Environment overrides remain supported; Linux
  uses environment keys or a secret manager.
- Source archives come from a clean committed tree after publication checks.
  CI has read-only repository permissions and a separate dependency audit.

Experimental source release candidate. No new paid provider validation or
comparative performance claim is included in these changes.

## 0.3.0-rc.5 — Preference self-repair

- Startup repairs the exact incomplete Claude model values an older, still-open
  process could write after the rc.4 migration, then atomically saves the correction.

## 0.3.0-rc.4 — Safer model selection

- Claude model shortcuts `/model opus` and `/model sonnet` expand to the complete
  configured model IDs.
- Incomplete model IDs ending in `-` fail locally before a paid provider request.

## 0.3.0-rc.3 — Conversational replies and stream recovery

- Exact greetings return to the composer without repository tools or provider calls.
- Informational answers use `awaiting_input`, without inventing tests or claiming
  verified coding completion. Coding `finish` still requires current verification.
- Claude HTTP/SSE failures retain sanitized error types, request IDs and known usage.
- Transient response retries are bounded by two attempts and existing request caps,
  respect cancellation/Retry-After, and never replay completed tools or partial actions.

## 0.3.0-rc.2 — Native conversations and bounded decisions

- Native conversations accept follow-ups, retaining exact prior requests and
  evidence with cumulative usage and fresh verification.
- Jev selection uses focused excerpts; retention batches fit both limits, skip
  infeasible or uneconomic work, and stop once the working set fits its target.
- New files can create approved parent directories with recovery; policy version 3.
- `/eviction` independently configures and remembers context retention policy.

- Claude uses streamed client-tool proposals with explicit output and effort budgets.
  A single concrete next action avoids unnecessary alternative implementations.
- Exact snippet replacements become validated, reviewable full patches before execution.
- Provider/model preferences persist; multiline input wraps and Keychain waits are bounded.
- Evicted patches no longer leak duplicate source through action arguments; canonical
  artifacts remain intact and exactly rehydratable.

- Distinct Claude stop reasons, bounded smaller-action recovery after output
  truncation, and preserved failure usage. No partial action is executed.
- Compact task display, preserved pasted newlines and visible tool/recovery activity.
- Direct native CLI reuses saved macOS Keychain credentials; interactive home
  supports `/permissions full-access`.
- Explicit native `--auto-approve` / `--full-access` grants consent for supported
  actions, persists on resume, logs each exact candidate/diff and remains visible.
  Denies, stale checks, path protections and cancellation remain enforced.
- One shared prompt and identical Git starters for an interactive official Claude
  Code versus native S1Code product demo; no model calls during preparation.
- Private launch records, remembered native credentials and separate local preview
  origins. Preview serves only game assets, not Git metadata or session records.
- Approval-required `node --test` supports dependency-free JavaScript verification.
  Extra Node flags, script execution and npm remain denied. Policy version 2
  invalidates old pending approvals; a new task uses the expanded command policy.

## 0.3.0-rc.1

Experimental source release candidate for macOS and Linux. No cost, speed or task
quality superiority claim is made.

- Context compaction reads each active artifact once per pass and plans changes
  before applying them. Overflow keeps the prior working set intact.
- Small evidence is retained when replacing it would increase context size.
- Jev retention batches carry only the evidence being scored, with real excerpts,
  diagnostic lines, capture revisions and dependencies.
- Native Claude requests place stable evidence before mutable metadata and use
  the documented five-minute prompt-cache breakpoint. Usage records distinguish
  cache writes and reads; savings and cache hits are not assumed.
- Generation asks for short user-facing updates; structured action details remain
  in the inspector. A sole concrete action takes precedence over the generation
  escape route in deterministic selection.
- Offline context probe, Unicode budget properties, transactional overflow and
  stable-prefix regressions supplement existing patch/resume/cancellation tests.

Existing session storage remains compatible. The new prompt contract invalidates
only repeated-planning fingerprints, not completed tool actions or approvals.
Provider-restricted live validation records remain private. Comparative cost and
latency benefits are not established. The official Codex bridge has protocol
checks; full live delegated coding validation is pending.
