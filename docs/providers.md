# Providers and authentication

## Native generation

OpenAI Responses HTTP endpoint: `https://api.openai.com/v1/responses`, API-key
Bearer authentication. `OPENAI_API_KEY` is read only by the generation adapter.
Requests use `stream: true`, `store: false`, a strict JSON schema, and at most 8192
output tokens. Model IDs are configurable. Local mock-server tests cover fragmented
UTF-8/SSE, structured proposals, usage and terminal validation. No billed Responses
call was made during this build; model/account availability is unverified.

The contract is `{"message":"visible plan","actions":[...]}` with 1–4 alternative
next actions, not a queued program. Actions are tagged `list`, `search`, `read`,
`patch`, `run`, `git`, `rehydrate`, `ask_generator`, `finish`, or `blocked`. See
`generation::proposal_schema` and `domain::Action` for exact arguments. Native
providers never execute tools. No hidden reasoning is requested or displayed.
Failures never trigger an implicit provider change or a Codex delegation.

## Jev decisions

`TYPESAFE_API_KEY` authenticates `POST https://api.typesafe.ai/v1/systemone` with
`model`, `state`, and a keyed `questions` map. Choice/Noul/Score answers and usage
are decoded and validated against the request. The runtime uses Choice for
ambiguous action relevance and Noul for optional context retention. Score is
available through the typed adapter and integration tests.

`--jev-model jev-1.13.0` is pinned. Moving aliases are rejected. Both documented
limits are enforced with conservative byte-count estimates plus envelope reserve:
`--jev-request-limit 64000` and `--jev-state-limit 32000`. These are **estimates**,
not an official tokenizer. Large focused states can still be rejected by Nerve.

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
OpenRouter keys cannot authenticate the direct TypeSafe endpoint. Nerve uses the
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
invented as returned model data. Distribution/confidence fields needed by Nerve's
policy must be present or the response is rejected. Reported gateway costs remain
attached to the response and are not treated as total task cost.

In zsh, enter the key without putting its value in shell history:

```sh
read -s 'OPENROUTER_API_KEY?OpenRouter API key: '
echo
export OPENROUTER_API_KEY
nerve run "fix the parser and run tests" --mode native \
  --decision jev --jev-provider openrouter --max-provider-requests 8
```

Native generation still requires `OPENAI_API_KEY`. Managed ChatGPT authentication
is available in Codex bridge mode, whose tool selection is owned by Codex; Jev
selection flags are rejected there instead of being silently ignored.

An explicit billed gateway contract check is available:

```sh
NERVE_LIVE_BUDGET_REQUESTS=1 cargo test --test live openrouter_live_contract -- --ignored
```

This check was not run during implementation. Local HTTP fixtures verify the wire
contract, build pin, missing-field errors and cache behavior. Alpha API support
is not a claim of successful live inference.

The [TypeSafe MCA](https://typesafe.ai/legal/mca) remains separate from Apache-2.0.
Keep service measurements private until documented clearance. Do not use service
outputs for imitation, distillation or competing model development. Renaming results
is not an exception. Nerve's export command refuses Jev traces.

## Official Codex bridge

Install the official CLI yourself. This build targets **codex-cli 0.153.3** and
fails closed on other versions until protocol schemas are checked.

```sh
nerve login codex
nerve account codex
nerve run "fix the parser" --mode codex
nerve resume <session-id>
nerve logout codex
```

Login invokes official `account/login/start` with `type: chatgpt`, displays the
managed authorization URL and waits for `account/login/completed`. Account status
omits email/plan identifiers. Logout invokes `account/logout` only when requested.
Nerve does not scrape tokens, implement OAuth, impersonate official clients or call
private subscription endpoints. Jev access is separate and is not included in this
login. No unlimited/free inference claim is made.

The local transport is stdio JSONL with initialize/initialized, thread/start or
thread/resume, turn/start, streaming notifications and turn/interrupt. Permission
settings start read-only with network disabled and all granular approval gates on.
The bridge checks the returned sandbox, working directory, approval policy and user
approval reviewer. The installed CLI requires `experimentalApi` capability for its
granular approval policy; Nerve opts in for that documented contract.
Native exclusions cannot be enforced upstream, so `--exclude` is rejected in bridge
mode. Approving an upstream command can allow execution beyond read-only sandbox;
the exact upstream request is shown. Persistent grants are unsupported.

Nerve never executes upstream tools itself. Duplicate approval request IDs reuse
only an identical reply; changed arguments are rejected. Unknown client requests
are rejected and permission-extension requests receive no grants. Headless bridge
approvals are denied; use the terminal for interactive approval. Saved turns are
observed on resume and are never blindly resubmitted. Active turns can be reattached;
a stopped turn requires explicit `resume ID --continue-task` to request another
turn on the same thread. Native request budgets cannot bound hidden upstream
inference; the bridge currently delegates one turn at a time. Internal calls/token counts
are unknown; a turn is a delegation, not one model call. Completion reporting only
notes observed upstream verification evidence, not proof of task correctness.

CLI protocol/account checks and local protocol fixtures are separate from billed
inference checks. See BUILD_STATUS.md for the actual verification results.

## Unsupported authentication

No Claude.ai subscription login, credential scraping, copied OAuth clients, or
Gemini/Copilot/Claude OAuth adapter exists. Future integrations must use supported
authentication and a real provider contract, not an empty backend for optics.
