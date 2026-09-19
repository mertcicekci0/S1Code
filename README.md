<div align="center">

# S1Code

**A native Rust coding agent with inspectable decisions and recoverable context.**

Plan with a generative model. Choose concrete actions. Review the changes. Resume the work.

[Get started](#get-started) · [How it works](#how-it-works) · [Documentation](#documentation) · [Contributing](CONTRIBUTING.md)

</div>

---

S1Code brings a coding agent into your terminal: describe a task, inspect its actions
and diff, approve changes, and see the actual verification result. Sessions and
captured evidence survive restarts.

Its native runtime combines **deterministic policy**, **bounded decisions**, and
**generative reasoning**. Claude or OpenAI handles planning and code generation.
Optional Jev integration selects among fully specified actions and can help decide
which evidence stays in the active context. S1Code owns local tool execution.

> **Source preview · 0.3.0-rc.6** — macOS and Linux. Built from source; no registry
> package or hosted binary release yet. See [implementation status](docs/BUILD_STATUS.md)
> for verified paths and remaining limits.

## What you can do

- **Work in a real repository.** Search and read source, apply validated patches,
  inspect Git changes, and run supported verification commands.
- **Keep the conversation readable.** Streaming replies and tool outcomes up front;
  candidates, decisions, context and diffs in an optional inspector.
- **Control execution.** Review exact patches and commands, or preapprove supported
  actions for a trusted workspace. Cancel from the keyboard.
- **Pick up where you left off.** Saved sessions retain requests, artifacts and
  activity. Resume without blindly repeating completed tools.
- **Recover evicted evidence.** Context eviction removes material from the working
  set while preserving captured bytes for rehydration without rerunning commands.
- **Use the same engine in scripts.** Headless mode emits JSONL and persists approval
  requests for later review.

## Get started

Requires Git and [Rust via rustup](https://rustup.rs/). The repository pins Rust
1.94.0. Python 3 runs the included parser demo; Windows is currently unsupported.

```sh
git clone https://github.com/mertcicekci0/S1Code.git
cd S1Code
cargo install --path . --locked
s1code doctor
```

### Native Claude + Jev

On macOS, save each API key once using a hidden prompt backed by login Keychain:

```sh
s1code auth set claude
s1code auth set typesafe
```

Open S1Code **inside the repository you want to work on**:

```sh
cd /path/to/your/project
s1code
```

Enter these commands in the S1Code input field, one at a time:

```text
/provider claude
/model opus
/jev typesafe
```

Then type your task directly:

```text
Find why the parser tests fail, make the smallest correct fix, and run the tests.
```

Provider and model choices are remembered. Keys are reused from Keychain; never
paste them into a task. On Linux, configure `ANTHROPIC_API_KEY` and
`TYPESAFE_API_KEY` through your environment or secret manager.

**Jev is optional.** `/decision rules` uses deterministic selection without a
learned decision backend. `/eviction jev` separately enables Jev-assisted context
eviction. Jev credentials and billing are independent of the generation provider.

### Managed ChatGPT login

The separate Codex bridge uses the official installed Codex CLI and its managed
login:

```sh
s1code login codex
s1code run "Investigate and fix the failing parser test" --mode codex
```

Codex owns execution, internal model turns and context in this mode. This is a
delegation to the official runtime, not the native S1Code loop. See
[provider setup](docs/providers.md) for CLI compatibility and permissions.

Native OpenAI generation is also available through the Responses API using an
OpenAI API key. `/claude-code` hands the terminal to the installed official Claude
Code application; that handoff does not run S1Code's native loop or Jev decisions.

## In the terminal

| Action | Command or key |
| --- | --- |
| Inspect activity, decisions, context and diff | `Ctrl+O` or `F2` during a task |
| Review the diff from the follow-up field | `/diff` |
| Return from the inspector | `Esc` |
| Cancel a running task | `Esc` or `Ctrl+C` |
| List saved tasks | `/sessions` on the home screen |
| Resume the latest task in this workspace | `/resume` on the home screen |
| Resume a particular task | `/resume ID` or a unique ID prefix |
| Show setup commands | `/help` |

Continue a task in its follow-up field to retain the conversation. A new prompt
from the home screen starts an independent saved task.

Native patches and processes require approval by default. For a trusted workspace,
`/permissions full-access` preapproves **supported actions** for the session;
`/permissions manual` restores prompts. Denied actions remain denied. This is not
an OS sandbox: approved tests and build scripts execute repository code with your
user's filesystem and network access.

## How it works

```text
Observe → construct concrete candidates → filter by policy → select
   ↑                                                        ↓
Verify and update ← persist evidence ← execute ← revalidate
```

The generator supplies plans, complete tool arguments, patches or answers. Forced
transitions run directly; ambiguous candidate selection can use Jev. Deterministic
policy and workspace checks apply again before execution, regardless of which
backend selected the action. Completion requires current verification evidence.

The active context is a working set over durable artifacts. Budget-triggered
**reversible eviction** preserves pinned constraints and dependency groups; evicted
content can be retrieved exactly as captured. Historical snapshots stay historical.
Rehydration preserves evidence, not a guarantee that a model will ask for it.

Claude prompt-cache usage, provider calls, tool actions and verification outcomes
are recorded when exposed. Lower cost, greater speed and better task performance
are hypotheses to evaluate, not promises.

## Try it without API keys

```sh
s1code demo --offline --workspace /tmp/s1code-parser-demo
s1code context-demo --workspace /tmp/s1code-context-demo
```

Use empty destination directories. The parser demo uses a labeled **OFFLINE
SIMULATION** for generation, with real reads, patch approvals and failing/passing
tests. The context demo stores, evicts and rehydrates actual evidence. Neither is
a live model demonstration.

For a disposable, budgeted **live Claude + Jev** task, run this from the source
checkout:

```sh
python3 scripts/try_live.py task
```

The launcher shows its request allowance before any API calls and reuses saved
macOS keys. See the [demo guide](docs/demo.md) for preparation and interpretation.

## Headless use

```sh
s1code run "Fix the failing parser test" --provider claude --decision jev --headless
s1code resume <session-id> --headless --approve <exact-candidate-id>
```

Stdout contains machine-readable JSONL; diagnostics go to stderr. Approval is bound
to the exact action and its preconditions. A run does not automatically commit or
push your changes.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
python3 scripts/release_check.py
```

Default tests need no credentials or network inference. `scripts/check.sh` runs the
broader local checks; [evaluation](docs/evaluation.md) explains fixture validation,
live trials and matched-policy comparisons. Fixture success is not agent success.
Provider-restricted results stay private pending clearance.

## Documentation

- [Quickstart](docs/quickstart.md) · [Providers and authentication](docs/providers.md)
- [Architecture](docs/architecture.md) · [Implementation status](docs/BUILD_STATUS.md)
- [Demo guide](docs/demo.md) · [Evaluation methodology](docs/evaluation.md)
- [Security](SECURITY.md) · [Privacy](PRIVACY.md) · [Contributing](CONTRIBUTING.md)
- [Changelog](CHANGELOG.md) · [Release process](docs/releasing.md)

## License

New project code is [Apache-2.0](LICENSE). Dependencies retain their own licenses;
see [third-party notices](THIRD_PARTY_NOTICES.md). No vendor affiliation or endorsement
is claimed. S1Code is a working name; naming availability and trademark clearance
have not been established.
