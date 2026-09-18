# Quickstart

From the source checkout, `cargo install --path . --locked` builds the pinned Rust
release binary. Run `s1code doctor` to check storage, credential presence, and CLI
compatibility. Doctor never prints keys.

Start with `s1code demo --offline --workspace /tmp/s1code-demo-1`. The parser fixture
starts broken. S1Code requests approval to run Python unittest, captures failures,
reads the parser, proposes a hash-bound replacement, shows the unified diff, and
requests patch approval. Approve a final test run to obtain actual verification.
This driver is simulated; it exercises real tools without paid inference.

For a real task, set `OPENAI_API_KEY` outside the repository, open the target
repository, and run `s1code run "your bounded task"`. The default policy is rules.
`--decision jev` also needs `TYPESAFE_API_KEY`. `--decision generative` uses the same
Responses model for constrained action selection and counts those generation calls.
Each mode defaults to 40 steps, 12 generation calls, and 24 total provider requests.
These are request caps, not dollar caps. Provider pricing/usage can vary.

If your Jev key is from OpenRouter, use `OPENROUTER_API_KEY` and
`--decision jev --jev-provider openrouter` instead. See providers.md for hidden key
entry and the gateway's pinned serving build. To use your ChatGPT account, run
`s1code account codex` or `s1code login codex`, then `s1code run "task" --mode codex`.
This delegates execution to Codex; it does not enable native Jev selection.

`--headless` emits only JSONL on stdout; diagnostics use stderr. Read an
`approval_required` event, inspect its action/diff and candidate ID, then run
`s1code resume ID --headless --approve CANDIDATE_ID`. A changed workspace invalidates
that approval. One ID approves one action. There is no blanket native approval flag.

`s1code sessions` lists IDs/statuses. `s1code resume ID` loads the recorded working
set and validates current files. A cancelled process or incomplete patch is treated
as uncertain. Inspect the trace and workspace first, then use
`s1code recover ID` for a retained patch journal and
`s1code resume ID --acknowledge-interruption` to explicitly clear uncertain intent.
Recovery refuses to overwrite subsequent user edits.

Terminal keys: `1` activity, `2` candidates/selection, `3` context, `4` diff;
`↑`/`↓` select events, `Enter` expand/collapse, `PgUp`/`PgDn` scroll,
`y`/`n` approve/deny, `Esc` or `Ctrl-C` cancel. After a run stops, press `q` to close
and leave a copyable summary in the normal terminal.

Approvals show the command or changed files in a dedicated card. Activity and
decision views show readable summaries; `j` explicitly toggles the raw event for
diagnostics. The offline demo header explicitly says that no model calls occur.

Use `--exclude relative/path` to protect additional native paths. Hidden files,
ignored files, symlinks, common secret names, dependency directories and `.git`
are unavailable to native file tools. See SECURITY.md for trusted-code limits.
