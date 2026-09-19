# Changelog

## Unreleased — Native usability and recovery

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
