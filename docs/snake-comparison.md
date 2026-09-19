# One prompt, two Snake websites

This is an interactive **product comparison**: official Claude Code versus native
S1Code using Claude + Jev. The initial source commit and user prompt are identical;
the harnesses, tools, system instructions and authentication differ. It does not
isolate Jev's effect. A small Snake task may never trigger context eviction.

## Prepare without inference

Requires Git with your configured human identity, Node.js 22+, Python 3, Rust and
the official Claude Code CLI. From the S1Code source checkout:

```sh
python3 scripts/snake_compare.py prepare
python3 scripts/snake_compare.py prompt
```

Preparation creates two private temporary repositories from the same starter
commit, without any game implementation. The placeholder test intentionally fails.
The shared prompt is [prompt.txt](../fixtures/snake/prompt.txt). No model is called.
The ignored `private/snake-comparison.json` pointer records the workspace location.

## Send the prompt once to each

Run each arm once, preferably sequentially to avoid CPU/network contention:

```sh
python3 scripts/snake_compare.py claude
python3 scripts/snake_compare.py s1code --max-requests 24 --full-access
```

Both commands send the exact shared prompt automatically, requesting claude-opus-5.
There is no need to paste a second prompt. The shown S1Code command explicitly auto-approves supported patches/tests;
its header says AUTO APPROVE. Omit `--full-access` for manual approval. Claude Code
keeps its own normal permission prompts. Record this permission difference when
comparing interaction time;
do not give either side hints that the other side does not receive. Count any extra
help or corrective prompt as an intervention. Starting an already-run arm is refused;
`prepare` creates a new independent trial instead of overwriting the previous one.

If needed, run `claude auth login` once before starting its arm. The launcher
checks account availability before marking a trial started.

Claude Code uses its own managed login in safe mode, with external MCP servers
disabled. Its subscription/API usage is not represented as S1Code usage or as zero
cost. S1Code uses native Anthropic and TypeSafe API keys, reusing saved macOS Keychain
credentials. The explicit request cap authorizes at most that many combined native
provider attempts, including retries; it is not a dollar cap. No credentials are
given to Cargo. Automatic approval applies only to supported native actions, with exact candidates
and diffs still recorded. It never permits npm, arbitrary shell, out-of-root access
or overrides a policy deny. It remains active when that native session is resumed.

The launcher records the initial commit, prompt hash, model requested, process exit
and interactive wall time privately. Wall time includes user idle/approval time;
exit code is not task success. Unavailable cost/usage stays null. Native canonical
events and usage live under the trial's `records/sessions` directory. No transcript
or benchmark is automatically published. Jev outputs remain private pending clearance.

## Play and review

In two terminals, serve the completed sites:

```sh
python3 scripts/snake_compare.py serve claude
python3 scripts/snake_compare.py serve s1code
```

Open http://127.0.0.1:8081 and http://127.0.0.1:8082. Separate origins avoid
sharing high-score storage. Servers bind only to loopback and expose only the
three game assets, not Git metadata, session records or keys. Ctrl-C stops them.

Use the same review checklist on both outputs, independently of their own tests:

- Start a game; arrow keys, WASD and touch buttons move it without page scrolling.
- Rapid opposite inputs between ticks cannot reverse the snake into itself.
- Food appears on free cells; eating grows the snake and adds ten points.
- Wall and self collisions end the game; pause freezes it and resume continues it.
- Restart resets the board/score and does not create duplicate movement timers.
- Best score survives reload; unavailable storage does not prevent playing.
- At 390px and desktop width, controls remain readable and usable without overflow.
- Read and run their `node --test` tests; reject vacuous tests and visible browser errors.

Record failures as failures, not as faster completion. One task/run is a demonstration,
not a cost/latency superiority result. A matched decision-policy evaluation and a
longer context-pressure task are separate experiments.
