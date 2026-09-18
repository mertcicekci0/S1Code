# Demonstration

## Interactive offline coding path

```sh
cargo build --locked
./target/debug/nerve --home /tmp/nerve-demo-store demo --offline \
  --workspace /tmp/nerve-parser-demo
```

Use new empty directories. The screen says OFFLINE SIMULATION throughout. Approve:
1. Python unittest, which genuinely fails on the initial parser.
2. The displayed one-file hash-bound patch.
3. Python unittest again, which genuinely verifies the new parser behavior.

The activity/decision/context/diff views show recorded actions and artifacts. This
is a reproducible harness demonstration, not live Jev or live generation. Generated
fixture edits are not automatically committed. Press Esc during a process to exercise
cancellation, then inspect the session with `nerve sessions` and resume it.

## Live native path

Copy `fixtures/demo/parser.py` and `test_parser.py` into an empty repository, set
`OPENAI_API_KEY`, and run:

```sh
nerve run "Fix parse_count for whole signed integers, blanks and invalid text; run tests" \
  --workspace /path/to/fixture --max-generations 8 --max-provider-requests 12
```

Adding `--decision jev` requires a separate TypeSafe key, or use
`--decision jev --jev-provider openrouter` with `OPENROUTER_API_KEY`.
Native generation still needs `OPENAI_API_KEY`. Provider calls may involve
planning, search formulation and debugging, not just patch generation. All count.
Live inference was not recorded during the build because keys were unavailable.

## Managed ChatGPT path

With the supported official Codex CLI installed, use your managed ChatGPT login.
From this source checkout:

```sh
./target/release/nerve account codex
# If an account is not connected:
./target/release/nerve login codex
nerve_trial=$(mktemp -d /tmp/nerve-codex-demo.XXXXXX)
cp fixtures/demo/*.py "$nerve_trial/"
git -C "$nerve_trial" init -q
./target/release/nerve run \
  "Fix parse_count for whole signed integers, blanks and invalid text; run tests" \
  --mode codex --workspace "$nerve_trial"
```

This is live delegated execution and uses your account's applicable limits. It is
not the offline driver and does not use Jev. Inspect each upstream approval in the
terminal. Local account/protocol checks passed during development; a full coding
turn has not been verified. See providers.md for ownership and permission limits.

## Real context pressure

```sh
nerve context-demo --workspace /tmp/nerve-context-demo
```

This creates harmless diagnostic files, captures five real read results, pins the
constraint, evicts eligible historical evidence under a bounded working-set budget,
and reads the artifact back byte-for-byte. No model answers are scripted. Rehydration
does not rerun a tool. This demonstrates recoverability, not lossless understanding.

## Headless approval, export and replay

```sh
nerve demo --offline --headless --workspace /tmp/nerve-headless-demo
nerve resume SESSION --headless --approve EXACT_CANDIDATE_ID
nerve export SESSION /tmp/nerve-demo.jsonl
nerve replay /tmp/nerve-demo.jsonl
```

Repeat resume with the next displayed approval ID. Exported events strip known keys
and personal paths; inspect before sharing. Every replay record carries its replay
label. Jev traces are deliberately blocked from this export path pending clearance.

For repeatable CLI and terminal checks after `cargo build --locked`, run
`python3 scripts/headless_smoke.py` and `python3 scripts/terminal_smoke.py`.
Both create disposable fixtures and authorize only their known demo actions.
They exercise the labeled offline driver with real tools; neither uses a provider.
