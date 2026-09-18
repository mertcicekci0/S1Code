# Nerve

A Rust terminal coding agent with concrete candidate actions, deterministic policy,
and recoverable context. **Experimental v0, built from source.** Nerve is a working
name; naming availability and trademark clearance have not been established.

Native mode owns the coding loop. A generation provider proposes plans and complete
next actions; rules or Jev select among candidates; Nerve validates, requests
approval, executes, and records evidence. Context eviction retains the captured
bytes for exact rehydration. This does not guarantee retained understanding or
better task performance.

The separate **Codex bridge** delegates execution to the official local Codex App
Server using its managed ChatGPT login. Its internal turns and context are outside
Nerve's control. Jev has separate credentials and billing.

## Install from source

Requires macOS or Linux, Git, Rust via rustup, and Python 3 for the included demo.
The pinned toolchain is Rust 1.94.0. Windows is unsupported. Linux CI is configured
but has not been observed running in this build session.

```sh
cargo build --locked --release
cargo install --path . --locked
nerve doctor
```

There is no published package or downloadable release advertised here.

## Try the actual tools offline

```sh
nerve demo --offline --workspace /tmp/nerve-parser-demo
```

The destination must be empty. The UI remains labeled **OFFLINE SIMULATION**:
generation is a deterministic fixture driver, while file reads, approval prompts,
patch application, failing/passing tests, and persistence are real. Press `y` to
approve the displayed action, `1`–`4` to inspect views, and `Esc` to cancel.

```sh
nerve context-demo --workspace /tmp/nerve-context-demo
nerve sessions
nerve resume <session-id>
```

The context demo performs real reads, eviction, and exact rehydration with no model.

## Native provider setup

Set `OPENAI_API_KEY` in your environment using your normal secret-management
workflow. No key is needed for offline tests. Do not put secrets in command history
or project files. The default generation model is `gpt-4.1-2025-04-14`; use `--model`
to select a Responses model supporting streaming structured output.

```sh
nerve run "fix the failing parser test" --mode native
nerve run "fix the failing parser test" --decision jev --max-provider-requests 12
```

Jev additionally needs `TYPESAFE_API_KEY`. It uses pinned `jev-1.13.0`. Native API
integrations have local HTTP contract tests; billed inference was **not verified**
in the build environment because API keys were unavailable. See
[implementation status](docs/BUILD_STATUS.md) before relying on a provider path.

Every native process and patch requires exact approval. Native execution is **not
an OS sandbox**: repository tests/build scripts execute code with your user's
filesystem/network access. Use trusted repositories or your own sandbox.

## Headless and delegated modes

```sh
nerve run "fix the failing parser test" --headless
nerve resume <session-id> --headless --approve <exact-candidate-id>
nerve login codex
nerve account codex
nerve run "fix the failing parser test" --mode codex
```

Headless stdout is JSONL. Native runs pause durably at approvals. The Codex bridge
uses the official CLI's credentials; Nerve never reads token files. Consult
[provider setup and mode boundaries](docs/providers.md) for compatibility and the
bridge's different approval/resume behavior.

## Validate and evaluate

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
nerve eval --suite fixtures/core
nerve eval --suite fixtures/heldout --output eval-results/heldout
```

Default evaluation validates the fixtures and reference patches, **not agent
performance**. Live trials are explicit and budgeted. Results remain private;
no benchmark wins, prices, or speed claims are made. See
[evaluation methodology](docs/evaluation.md), [demo](docs/demo.md),
[architecture](docs/architecture.md), [security](SECURITY.md), and
[privacy](PRIVACY.md).

New project code is Apache-2.0. Dependencies retain their licenses. No vendor
endorsement or affiliation is claimed.
