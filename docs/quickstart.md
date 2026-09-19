# Quickstart

From the source checkout, `cargo install --path . --locked` builds the pinned Rust
release binary. Run `s1code doctor` to check storage, credential presence, and CLI
compatibility. Doctor never prints keys.

Run `s1code` for the interactive task-entry screen. `/login` connects the official
Codex bridge to your ChatGPT account, `F2` changes provider, and `/help` shows setup.
On macOS, native Claude/Jev automatically reuse keys previously saved by the
launcher in login Keychain. No export commands are required; macOS may request
Keychain access. Explicit environment keys take precedence. Keys stay in provider
memory and are not added to child-process environments. On other platforms,
configure ANTHROPIC_API_KEY and the Jev key. To use native Claude with Jev,
then enter `/provider claude` and `/jev typesafe` (or `/jev openrouter` for that gateway).
Use `/model claude-opus-5` to select your model. Optional `/permissions full-access`
preauthorizes supported native actions; `/permissions manual` restores prompts.
Changing provider resets this choice. Paste your task directly into the input and
press Enter; no shell clipboard command is needed.
After a task, `q` closes its activity view and returns home. New prompts create
independent saved tasks; `/resume ID` reopens an existing one. `/claude-code` hands
the terminal to the official Claude Code application and does not use Jev.

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
that approval. One ID approves one action. For trusted native sessions, explicitly
start with `--auto-approve` (alias `--full-access`) to preauthorize supported
patches/tests. This setting persists on resume; policy denies and stale-action
checks remain enforced. It does not enable arbitrary shell commands.

`s1code sessions` lists IDs/statuses. `s1code resume ID` loads the recorded working
set and validates current files. A cancelled process or incomplete patch is treated
as uncertain. Inspect the trace and workspace first, then use
`s1code recover ID` for a retained patch journal and
`s1code resume ID --acknowledge-interruption` to explicitly clear uncertain intent.
Recovery refuses to overwrite subsequent user edits.

The default task screen shows conversation, not protocol events. `F2` toggles the
inspector. In a Codex conversation, type a follow-up and press Enter after a response;
Esc closes it without submitting another turn. Interactive resume waits for input.
Native runs finish after their bounded task.

Inspector keys: `1` activity, `2` candidates/selection, `3` context, `4` diff;
`↑`/`↓` select events, `Enter` expand/collapse, `PgUp`/`PgDn` scroll,
`y`/`n` approve/deny, `Esc` or `Ctrl-C` cancel. After a run stops, press `q` to close
and leave a copyable summary in the normal terminal.

Approvals show the command or changed files in a dedicated card. Activity and
decision views show readable summaries; `j` explicitly toggles the raw event for
diagnostics. The offline demo header explicitly says that no model calls occur.

Use `--exclude relative/path` to protect additional native paths. Hidden files,
ignored files, symlinks, common secret names, dependency directories and `.git`
are unavailable to native file tools. See SECURITY.md for trusted-code limits.

Provider, model, effort and decision preferences are saved on leaving the entry
screen; reopen `s1code` in any project to reuse them. Workspace always starts at the
current directory and automatic approval is not saved as a global preference.
Use Shift-Enter for a newline or paste a multiline task. `/output-limit 16384` and
`/effort medium` configure native generation; Opus/Sonnet 5 already default to medium.
