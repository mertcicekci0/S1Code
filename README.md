# S1Code

A Rust terminal coding agent with concrete candidate actions, deterministic policy,
and recoverable context. **Experimental v0, built from source.** S1Code is a working
name; naming availability and trademark clearance have not been established.

Native mode owns the coding loop. A generation provider proposes plans and complete
next actions; rules or Jev select among candidates; S1Code validates, requests
approval, executes, and records evidence. Context eviction retains the captured
bytes for exact rehydration. This does not guarantee retained understanding or
better task performance.

The separate **Codex bridge** delegates execution to the official local Codex App
Server using its managed ChatGPT login. Its internal turns and context are outside
S1Code's control. Jev has separate credentials and billing.

## Install from source

Requires macOS or Linux, Git, Rust via rustup, and Python 3 for the included demo.
The pinned toolchain is Rust 1.94.0. Windows is unsupported. Linux CI is configured
but has not been observed running in this build session.

```sh
cargo build --locked --release
cargo install --path . --locked
s1code doctor
```

There is no published package or downloadable release advertised here.

## Start the interactive app

```sh
s1code
```

This opens a task-entry screen, not a demo. Type a task and press Enter. The default
is the explicitly labeled Codex bridge with managed ChatGPT login; `/login` starts
its official login flow. `F2` cycles providers. `/provider claude` selects native
Claude API generation, `/jev openrouter` enables Jev decisions in native mode,
`/sessions` lists saved tasks, and `/help` explains setup. Keys are read from the
environment, never from chat messages. Closing a task view returns to task entry;
each new prompt starts an independent saved task rather than silently inheriting
another task's permissions or context. Use `/resume ID` for existing work.

`/claude-code` (or `s1code claude-code`) opens the installed, unmodified official
Claude Code terminal. That application owns login, permissions, tools and history;
S1Code and Jev do not control or record its internal actions. Exit it to return.
This handoff is separate from native Claude generation.

## Try the actual tools offline

```sh
s1code demo --offline --workspace /tmp/s1code-parser-demo
```

The destination must be empty. The UI remains labeled **OFFLINE SIMULATION**:
generation is a deterministic fixture driver, while file reads, approval prompts,
patch application, failing/passing tests, and persistence are real. Press `y` to
approve the displayed action, `1`–`4` to inspect views, and `Esc` to cancel.

```sh
s1code context-demo --workspace /tmp/s1code-context-demo
s1code sessions
s1code resume <session-id>
```

The context demo performs real reads, eviction, and exact rehydration with no model.

## Native provider setup

For OpenAI, set `OPENAI_API_KEY` in your environment using your normal secret-management
workflow. No key is needed for offline tests. Do not put secrets in command history
or project files. The default generation model is `gpt-4.1-2025-04-14`; use `--model`
to select a Responses model supporting streaming structured output.

```sh
s1code run "fix the failing parser test" --mode native
s1code run "fix the failing parser test" --decision jev --max-provider-requests 12
```

Jev additionally needs `TYPESAFE_API_KEY`. It uses pinned `jev-1.13.0`. Native API
integrations have local HTTP contract tests; billed inference was **not verified**
in the build environment because API keys were unavailable. See
[implementation status](docs/BUILD_STATUS.md) before relying on a provider path.

For an OpenRouter Jev key, use `OPENROUTER_API_KEY` and add
`--decision jev --jev-provider openrouter`. This uses the dedicated Decisions API
with a checked serving build, not chat completions. Native generation still needs
its selected provider API key (OpenAI or Anthropic). Managed ChatGPT login belongs to Codex bridge mode.

Every native process and patch requires exact approval. Native execution is **not
an OS sandbox**: repository tests/build scripts execute code with your user's
filesystem/network access. Use trusted repositories or your own sandbox.

For native Claude, set `ANTHROPIC_API_KEY`, then:

```sh
s1code run "fix the failing parser test" --provider claude
s1code run "fix the failing parser test" --provider claude \
  --decision jev --jev-provider openrouter
```

The Claude adapter uses the public streaming Messages API with structured proposals.
The default model is `claude-sonnet-5`; `--model` selects an explicit model. Local
HTTP fixtures pass, including a real approved patch/test cycle. Billed Claude
inference has not been verified. Claude subscription tokens are never imported.

## Headless and delegated modes

```sh
s1code run "fix the failing parser test" --headless
s1code resume <session-id> --headless --approve <exact-candidate-id>
s1code login codex
s1code account codex
s1code run "fix the failing parser test" --mode codex
```

Headless stdout is JSONL. Native runs pause durably at approvals. The Codex bridge
uses the official CLI's credentials; S1Code never reads token files. Consult
[provider setup and mode boundaries](docs/providers.md) for compatibility and the
bridge's different approval/resume behavior.

## Validate and evaluate

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
s1code eval --suite fixtures/core
s1code eval --suite fixtures/heldout --output eval-results/heldout
```

Default evaluation validates the fixtures and reference patches, **not agent
performance**. Live trials are explicit and budgeted. Results remain private;
no benchmark wins, prices, or speed claims are made. See
[evaluation methodology](docs/evaluation.md), [demo](docs/demo.md),
[architecture](docs/architecture.md), [security](SECURITY.md), and
[privacy](PRIVACY.md).

New project code is Apache-2.0. Dependencies retain their licenses. No vendor
endorsement or affiliation is claimed.
