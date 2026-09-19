# Providers and authentication

## Native generation

OpenAI Responses HTTP endpoint: `https://api.openai.com/v1/responses`, API-key
Bearer authentication. `OPENAI_API_KEY` is read only by the generation adapter.
Requests use `stream: true`, `store: false`, a strict JSON schema, and a configurable
output limit (16384 tokens by default). Model IDs are configurable. Local mock-server tests cover fragmented
UTF-8/SSE, structured proposals, usage and terminal validation. No billed Responses
call was made during this build; model/account availability is unverified.

The contract is `{"message":"visible plan","actions":[...]}` with 1–4 alternative
next actions, not a queued program. Actions are tagged `list`, `search`, `read`,
`patch`, `replace`, `run`, `git`, `rehydrate`, `ask_generator`, `finish`, or `blocked`. See
`generation::proposal_schema` and `domain::Action` for exact arguments. Native
providers never execute tools. No hidden reasoning is requested or displayed.
Failures never trigger an implicit provider change or a Codex delegation.

## Native Claude generation

Choose `--provider claude` (or `/provider claude` on the home screen) and set
`ANTHROPIC_API_KEY`. S1Code calls `POST https://api.anthropic.com/v1/messages` with
`x-api-key`, `anthropic-version: 2023-06-01`, `stream: true`, and official
client tool definitions. Streamed `tool_use` arguments become validated internal
actions only after successful completion. The provider executes no tools. There is no hidden Claude Code runtime
in this mode. Jev works with either native generation provider.

The configurable default is `claude-opus-5`, listed in the official model docs
reviewed on 2026-09-19. Availability depends on the account. Private live acceptance records are not
published with this source release.
The adapter checks message/block ordering, fragmented UTF-8/SSE, final stop reason,
typed tool proposals, bounded output and cancellation. Refusal, truncated output,
HTTP/stream errors, unexpected tools and incomplete streams cannot create actions.
Thinking content is never shown or stored. Failures do not switch providers.

Usage retains provider-reported fields: Claude `input_tokens` excludes cached reads
and cache creation; these are separately recorded as `cached_input_tokens` and
`cache_creation_input_tokens`. OpenAI input tokens include its cached subset.
Do not add overlapping fields or compare raw input counts without normalization.
Missing counts stay unknown. Output deltas carry cumulative counts, not additions.
Native requests have a 180-second deadline and are not automatically retried.

```sh
read -s 'ANTHROPIC_API_KEY?Anthropic API key: '
echo
export ANTHROPIC_API_KEY
s1code run "fix the parser and run tests" --provider claude \
  --decision jev --jev-provider openrouter --max-provider-requests 8
```

The Jev key is separate. A single billed Claude contract check requires explicit
consent: `S1CODE_LIVE_BUDGET_REQUESTS=1 cargo test --test live claude_live_contract -- --ignored`.
This check was not run during development; it executes no proposed local actions.

## Jev decisions

`TYPESAFE_API_KEY` authenticates `POST https://api.typesafe.ai/v1/systemone` with
`model`, `state`, and a keyed `questions` map. Choice/Noul/Score answers and usage
are decoded and validated against the request. The runtime uses Choice for
ambiguous action relevance and Noul for optional context retention. Score is
available through the typed adapter and integration tests.

`--jev-model jev-1.13.0` is pinned. Moving aliases are rejected. Both documented
limits are enforced with conservative byte-count estimates plus envelope reserve:
`--jev-request-limit 64000` and `--jev-state-limit 32000`. These are **estimates**,
not an official tokenizer. Large focused states can still be rejected by S1Code.

The adapter allows at most three attempts. It honors Retry-After, uses bounded
backoff/jitter for 429/529 and appropriate server/transport failures, and does not
retry authentication or validation failures. Cancellation interrupts HTTP waits.
Exact-cache identity includes state, candidates, model, rubric, policy and workspace
revision. Cache entries live within one run process, never across changed tasks.

`--jev-confidence` controls an experimental Choice escalation threshold. The
selected option's probability and distribution-derived confidence are separate.
Noul has no confidence field. No score can authorize a tool or establish correctness.
`--jev-retention-threshold` controls the separate experimental Noul retention cutoff
(default 0.5). Both thresholds must be finite values from 0 to 1.
`--jev-fallback-rules` explicitly permits a deterministic fallback after Jev failure;
default is to stop. It does not send code to another provider.

`--eviction conservative|jev|off` is independent of selection policy. Jev eviction
batches independent retention questions against one snapshot with real excerpts.
Dependent decisions are evaluated after observing new state.

### Jev through OpenRouter

Use `OPENROUTER_API_KEY` with `--decision jev --jev-provider openrouter`.
OpenRouter keys cannot authenticate the direct TypeSafe endpoint. S1Code uses the
official alpha Decisions endpoint, `https://openrouter.ai/api/alpha/decisions`,
with `model`, `state`, and `questions`, not the chat-completions endpoint.
Requests restrict routing to TypeSafe and disable provider fallback. No automatic
switch between direct TypeSafe and OpenRouter is performed.

The request model is `typesafe/jev-1.13`; the serving build is pinned to
`typesafe/jev-1.13-20260917`, observed through OpenRouter's public endpoint metadata
on 2026-09-19. `--jev-resolved-model` changes the expected dated serving build
explicitly. A different returned build is rejected. The gateway's 32,000-token
context limit caps both conservative request budgets. Score legends may be omitted
by the gateway; their meaning is retained in the original question rubric, not
invented as returned model data. Distribution/confidence fields needed by S1Code's
policy must be present or the response is rejected. Reported gateway costs remain
attached to the response and are not treated as total task cost.

In zsh, enter the key without putting its value in shell history:

```sh
read -s 'OPENROUTER_API_KEY?OpenRouter API key: '
echo
export OPENROUTER_API_KEY
s1code run "fix the parser and run tests" --mode native \
  --decision jev --jev-provider openrouter --max-provider-requests 8
```

Native generation requires `OPENAI_API_KEY` for OpenAI or `ANTHROPIC_API_KEY` for Claude. Managed ChatGPT authentication
is available in Codex bridge mode, whose tool selection is owned by Codex; Jev
selection flags are rejected there instead of being silently ignored.

An explicit billed gateway contract check is available:

```sh
S1CODE_LIVE_BUDGET_REQUESTS=1 cargo test --test live openrouter_live_contract -- --ignored
```

This check was not run during implementation. Local HTTP fixtures verify the wire
contract, build pin, missing-field errors and cache behavior. Alpha API support
is not a claim of successful live inference.

The [TypeSafe MCA](https://typesafe.ai/legal/mca) remains separate from Apache-2.0.
Keep service measurements private until documented clearance. Do not use service
outputs for imitation, distillation or competing model development. Renaming results
is not an exception. S1Code's export command refuses Jev traces.

## Official Codex bridge

Install the official CLI yourself. This build targets **codex-cli 0.153.3** and
fails closed on other versions until protocol schemas are checked.

```sh
s1code login codex
s1code account codex
s1code run "fix the parser" --mode codex
s1code resume <session-id>
s1code logout codex
```

Login invokes official `account/login/start` with `type: chatgpt`, displays the
managed authorization URL and waits for `account/login/completed`. Account status
omits email/plan identifiers. Logout invokes `account/logout` only when requested.
S1Code does not scrape tokens, implement OAuth, impersonate official clients or call
private subscription endpoints. Jev access is separate and is not included in this
login. No unlimited/free inference claim is made.

The local transport is stdio JSONL with initialize/initialized, thread/start or
thread/resume, turn/start, streaming notifications and turn/interrupt. Permission
settings start read-only with network disabled and all granular approval gates on.
The bridge checks the returned sandbox, working directory, approval policy and user
approval reviewer. The installed CLI requires `experimentalApi` capability for its
granular approval policy; S1Code opts in for that documented contract.
Native exclusions cannot be enforced upstream, so `--exclude` is rejected in bridge
mode. Approving an upstream command can allow execution beyond read-only sandbox;
the exact upstream request is shown. Persistent grants are unsupported.

S1Code never executes upstream tools itself. Duplicate approval request IDs reuse
only an identical reply; changed arguments are rejected. Unknown client requests
are rejected and permission-extension requests receive no grants. Headless bridge
approvals are denied; use the terminal for interactive approval. Saved turns are
observed on resume and are never blindly resubmitted. Active turns can be reattached;
an interactive stopped turn waits for a new message, and Enter explicitly requests
one additional turn on the same thread. Headless continuation requires explicit
`resume ID --continue-task`. Interactive follow-ups reuse one App Server process;
resuming a closed conversation starts a new process and resumes the saved thread.
A completed upstream response without an observed check is `awaiting_input`, not
verified task completion. Native request budgets cannot bound hidden upstream
inference; the bridge currently delegates one turn at a time. Internal calls/token counts
are unknown; a turn is a delegation, not one model call. Completion reporting only
notes observed upstream verification evidence, not proof of task correctness.

CLI protocol/account checks and local protocol fixtures are separate from billed
inference checks. See BUILD_STATUS.md for the actual verification results.

## Official Claude Code terminal handoff

`s1code claude-code --workspace PATH` or `/claude-code` opens the installed official
`claude` executable unchanged, interactively, with no task or permission-bypass flags.
Its own sign-in flow can use the user's eligible subscription or API key. S1Code
neither reads nor stores its tokens, implements Claude.ai login, nor routes model
requests through subscription credentials. The terminal returns after Claude Code
exits. This is a handoff, not the native loop or a persisted S1Code bridge: Jev is
inactive, and tools, approval settings, sandbox, telemetry, history and resumption
belong to Claude Code. Use Claude Code's own controls. No S1Code metrics are invented.
Unrelated provider keys are removed from the child environment; an explicitly
configured ANTHROPIC_API_KEY is available to the official client that needs it.
The local `claude --version` reported 2.1.266; no billed Claude Code task was run.

The current [Anthropic authentication rules](https://code.claude.com/docs/en/legal-and-compliance)
distinguish end users signing into the unmodified official binary from third-party
applications collecting tokens or routing requests through subscription accounts.
Native S1Code Claude generation uses the public API, not a subscription-token adapter.
Jev always uses its separately billed TypeSafe/OpenRouter service. There is no built-in
mode where Jev replaces Codex or Claude Code's internal action selection.

## Unsupported authentication

No credential scraping, copied OAuth clients, third-party Claude.ai login, or
Gemini/Copilot OAuth adapter exists. Cloud-native Claude API authentication is not
implemented; this release's native Claude adapter uses ANTHROPIC_API_KEY.

## Local hidden-key live launcher

From the source directory, run `python3 scripts/try_live.py check` for at most one
Claude request and one official TypeSafe Jev request, or `python3 scripts/try_live.py task`
for a fresh parser task capped at eight total provider requests including retries.
The launcher builds before asking for keys, asks for explicit request-budget consent,
then reuses saved macOS Keychain credentials or reads missing keys without echo.
On macOS, newly entered keys are saved under `s1code.credentials.v1` for the next run.
Keys are never sent to Cargo/build scripts or stored in session files. A request cap is not a dollar cap. Set provider credit
limits separately. Task sessions remain in the printed private temporary directory;
keep Jev measurements private. These commands were prepared and tested offline;
live provider validation remains unverified until you run them successfully.

The launcher now defaults to direct TypeSafe access (`TYPESAFE_API_KEY`). Use
`python3 scripts/try_live.py jev-check` to test only Jev with one authorized request,
without a Claude key or request. Paste the full secret, not the dashboard's masked
preview. TypeSafe secrets may start with `apikey_`; the launcher does not confuse
them with Anthropic key IDs. To explicitly select the gateway instead, append
`--jev-provider openrouter` and enter its separate credential. Existing sessions
retain their original provider; the route is never silently changed on resume.

New native Claude runs now default to Opus 5. The launcher accepts `--model
claude-opus-5` for either `check` or `task`, and passes the same selection to the
actual generation adapter. The selected model is displayed before budget consent.
Existing sessions retain their original model. Changing the model is experimental;
it does not establish better task success, latency or cost. The eight-request task
cap remains unchanged and is not a dollar cap.

Credential management for this source launcher:

- `python3 scripts/try_live.py --forget-keys` deletes only its three provider records
  from Keychain and exits without API calls. Use this before replacing saved keys.
- `--no-keychain` uses environment credentials or temporary hidden input without
  saving. Linux currently uses this nonpersistent behavior automatically.
- Explicit environment keys take precedence and are not copied into Keychain.
- A denied/locked Keychain produces an error; there is no plaintext file fallback.
- macOS may ask permission for the Python interpreter to access saved records. This
  is system credential storage, not isolation from other authorized local programs.

The standalone Rust binary also reads these saved records automatically when an
explicit environment key is absent. macOS may request Keychain access for the system
security utility. The binary retains retrieved keys in memory, without adding them
to child-process environments. The launcher supplies only needed keys to its child. Keys are absent from command arguments and build/tool environments.

A native session's total budget can be explicitly updated with `s1code resume ID
--max-provider-requests 16 --max-generations 12`. These are total session caps,
including already-used requests, not extra allowances. The update is recorded and
counters are never reset. Caps cannot be lower than usage already recorded. This is
unsupported for delegated Codex inference whose internal calls remain unknown.

Claude output recovery: `max_tokens` is recorded separately from refusal. Native
planning permits at most one retry asking for one smaller action, inside the same
request/generation caps; usage from both completed responses is counted. Refusals,
unknown stop reasons and transport-truncated streams do not trigger that retry.
Partial proposals are discarded, never executed or joined to later JSON.

Native Claude uses public client-tool proposals, not a schema-constrained text
packet with nullable arguments for every action. S1Code executes selected tools.
`--max-output-tokens 16384` controls the output ceiling; `/output-limit 16384` sets
it in the home screen. `--effort medium` or `/effort medium` selects Claude effort;
Opus 5 and Sonnet 5 default to medium. Other models retain provider defaults, and
unsupported explicit levels may be rejected by that provider. Thinking shares the
output ceiling; its reported token count is a subset, not an extra total.
