# Demonstration

## Normal task entry

Run `./target/release/s1code` to open the real task-entry screen. This is not a
simulation. Opening it makes no model call; submitting a task uses the visibly
selected provider. `/demo` explicitly enters the offline fixture in a fresh temporary
workspace, and closing its task view returns home. `/help` lists provider setup.

## Interactive offline coding path

```sh
cargo build --locked
./target/debug/s1code --home /tmp/s1code-demo-store demo --offline \
  --workspace /tmp/s1code-parser-demo
```

Use new empty directories. The screen says OFFLINE SIMULATION throughout. Approve:
1. Python unittest, which genuinely fails on the initial parser.
2. The displayed one-file hash-bound patch.
3. Python unittest again, which genuinely verifies the new parser behavior.

The activity/decision/context/diff views show recorded actions and artifacts. This
is a reproducible harness demonstration, not live Jev or live generation. Generated
fixture edits are not automatically committed. Press Esc during a process to exercise
cancellation, then inspect the session with `s1code sessions` and resume it.

## Live native path

Copy `fixtures/demo/parser.py` and `test_parser.py` into an empty repository, set
`OPENAI_API_KEY`, and run:

```sh
s1code run "Fix parse_count for whole signed integers, blanks and invalid text; run tests" \
  --workspace /path/to/fixture --max-generations 8 --max-provider-requests 12
```

Adding `--decision jev` requires a separate TypeSafe key, or use
`--decision jev --jev-provider openrouter` with `OPENROUTER_API_KEY`.
Native generation still needs `OPENAI_API_KEY`. Provider calls may involve
planning, search formulation and debugging, not just patch generation. All count.
Provider-restricted live diagnostics remain private. Do not publish Jev traces or
service measurements without documented clearance; a passing contract check alone
does not establish successful coding.

## Managed ChatGPT path

With the supported official Codex CLI installed, use your managed ChatGPT login.
From this source checkout:

```sh
./target/release/s1code account codex
# If an account is not connected:
./target/release/s1code login codex
s1code_trial=$(mktemp -d /tmp/s1code-codex-demo.XXXXXX)
cp fixtures/demo/*.py "$s1code_trial/"
git -C "$s1code_trial" init -q
./target/release/s1code run \
  "Fix parse_count for whole signed integers, blanks and invalid text; run tests" \
  --mode codex --workspace "$s1code_trial"
```

This is live delegated execution and uses your account's applicable limits. It is
not the offline driver and does not use Jev. Inspect each upstream approval in the
terminal. Local account/protocol checks passed during development; a full coding
turn has not been verified. See providers.md for ownership and permission limits.

## Real context pressure

```sh
s1code context-demo --workspace /tmp/s1code-context-demo
```

This creates harmless diagnostic files, captures five real read results, pins the
constraint, evicts eligible historical evidence under a bounded working-set budget,
and reads the artifact back byte-for-byte. No model answers are scripted. Rehydration
does not rerun a tool. This demonstrates recoverability, not lossless understanding.

## Headless approval, export and replay

```sh
s1code demo --offline --headless --workspace /tmp/s1code-headless-demo
s1code resume SESSION --headless --approve EXACT_CANDIDATE_ID
s1code export SESSION /tmp/s1code-demo.jsonl
s1code replay /tmp/s1code-demo.jsonl
```

Repeat resume with the next displayed approval ID. Exported events strip known keys
and personal paths; inspect before sharing. Every replay record carries its replay
label. Jev traces are deliberately blocked from this export path pending clearance.

For repeatable CLI and terminal checks after `cargo build --locked`, run
`python3 scripts/headless_smoke.py` and `python3 scripts/terminal_smoke.py`.
The additional `python3 scripts/home_smoke.py` checks task entry and return, missing
credentials, resize, and a clearly labeled process stub for the external handoff.
These checks create disposable fixtures and authorize only their known demo actions.
They exercise the labeled offline driver with real tools; neither uses a provider.
