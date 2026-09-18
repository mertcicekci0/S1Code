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

Adding `--decision jev` requires a separate TypeSafe key. Provider calls may involve
planning, search formulation and debugging, not just patch generation. All count.
Live inference was not recorded during the build because keys were unavailable.

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
