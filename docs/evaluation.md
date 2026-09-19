# Evaluation methodology

`s1code eval --suite fixtures/core` is offline evaluator validation. It copies only
agent-visible starting files into a disposable workspace, runs independently stored
checks to confirm the task initially fails, applies the reference patch through the
real patch validator, and reruns checks. Its result is `fixture_validated_not_agent_success`;
`agent_success` is null. It says nothing about model speed, quality or price.

Five development fixtures cover a localized parser bug, a two-file API change,
a small feature with unhashable inputs, a misleading historical diagnostic, and
context pressure. `fixtures/heldout` is a separate task not used to implement the
fixture-specific demo driver; it is published for reproducibility, not a claim of
unseen data for every future contributor/model. Refresh it before serious evaluation.

Protected check scripts are created in a separate controller directory outside the
native tool root. Reference patches are held by the evaluator, never supplied to
the generation/decision state. The task's public tests can be changed, but protected
checks are independently run afterward. This is not an adversarial evaluator sandbox:
approved repository code is trusted. A hostile agent/program could attack same-user
processes/files. Do not use this harness for untrusted benchmark contestants.

For billed trials, review provider terms and authorize a bounded number of HTTP
attempts plus repository code execution:

```sh
s1code eval --suite fixtures/core --live --approve-fixture-execution \
  --live-budget-requests 120 --decisions rules,jev,generative --repeat 3 \
  --model gpt-4.1-2025-04-14 --eviction conservative \
  --output eval-results/private-comparison
```

This is a request spend limit, not a dollar guarantee. Missing credentials fail the
check; they never count as successful live validation. The global cap includes
retries and both generation and decision HTTP attempts. Each trial has the same
request cap (24 or the smaller initially authorized budget) and generation/step
budgets. The runner stops before starting a trial that cannot fit that cap. The rules policy is deterministic, not a
learned model. The constrained generative baseline sees the same candidates/evidence,
selects one exact action through the same model and benefits from the same tools and
provider prefix caching. Its selection calls count as generation.

Trials use deterministic shuffled order (default seed 42). Each report records the
starting file-tree hash; fixture Git commits are not fabricated. Generation model,
Jev version, settings, source Git commit when available, binary version, platform,
CPU concurrency, repeat number and actual sample count are recorded. Raw trials
permit inspecting variability. Do not report percentiles from tiny samples.

Metrics distinguish actual generation calls, decision attempts/questions, local tool
calls, retries, cache hits, context invalidations and rehydration. Returned token usage
and cached input tokens are retained; unknown usage/cost is null. Wall time is measured
from a monotonic clock and is not a sum of overlapping spans. Offline reference-check
timing is not agent performance. The bridge reports delegations with internal calls
unknown, and is not silently substituted for native trials.

Test context eviction separately using identical decision policy/model and changing
only `--eviction`. `s1code context-demo` verifies real eviction/rehydration mechanics
without a model or scripted scoring answers. Retain task-success evidence alongside
context size changes and cached token observations; prefix invalidations may cost
more than eviction saves.

`--jev-provider openrouter` selects the explicit gateway route for Jev trials and
requires `OPENROUTER_API_KEY`. Reports record the route, request model and expected
dated serving model; `--jev-resolved-model` changes that expected build explicitly.
Do not mix gateway and direct-provider trials in a controlled comparison without
recording the change. The generation provider remains matched across policies.

Live trial traces retain events, checkpoints and hashed artifacts together. Output
directories have private permissions and an ignore file; `eval-results/` is also
gitignored at the project root. Do not commit
or publish Jev measurements without documented provider clearance. The evaluator
ships no comparative performance table and no claim of lower cost or latency.

Native Claude comparisons use `--provider claude` and an explicit `--model` when
running `s1code eval --live ...`; set ANTHROPIC_API_KEY instead of OPENAI_API_KEY.
All compared decision policies share that provider/model. Reports include the
provider. Claude input tokens exclude cached reads/writes, which are recorded
separately; OpenAI input tokens include its cached subset. Unknown fields remain
null. Do not compare/sum these fields without normalizing their semantics.
The official Claude Code terminal handoff is outside this evaluator; its turns,
usage, approvals and success are not recorded as native S1Code measurements.

## Offline context mechanics probe

```sh
cargo run --locked --release --example context_probe
```

This creates real content-addressed artifacts, compacts 16/64/256-entry working
sets five times each, verifies exact rehydration, and emits JSON with serialized
byte counts and raw local wall-time samples. Repeated trials share filesystem
caches; fixture setup is excluded. Tokens and cost are null; provider calls are
zero. The previous-layout payload is the calculated size of repeatedly sending all
eligible excerpts, not a measured competing implementation or a service benchmark.
The probe cannot establish coding quality, billable-token savings or faster tasks.

`native_context_pressure_live` is an ignored integration test for authorized live
validation. It requires `S1CODE_LIVE_BUDGET_REQUESTS` (capped at 16), selected native
provider keys and an empty private `S1CODE_LIVE_OUTPUT`. It performs real bounded
reads to populate historical evidence, asks real Jev retention questions, runs
the native coding loop, permits only parser.py patches and exact fixture unittest
commands, verifies unchanged tests, and rehydrates from storage after reopening the
session. No scripted model answers. Compile before loading keys; run only that test
binary with `--ignored --exact native_context_pressure_live`. Keep all outputs private.

The 0.3.0-rc.1 Claude request layout and prompt contract are versioned with source.
Cache writes and reads remain separate usage fields and must both be included in
any cost analysis. Compare policies at the same source commit and model; do not
attribute simultaneous caching and eviction changes solely to Jev.

For 0.3.0-rc.2, Jev relevance selection uses focused partial excerpts and candidate
IDs, while the constrained generative baseline receives full candidate arguments
so it can return an exact action. This is a decision-policy/harness comparison, not
an isolated comparison of model weights on identical prompts. Record the source
revision and prompt/serialization contract with every trial. Decision byte counters
include retries, exclude exact cache hits, and are not token or price estimates;
historical sessions without complete accounting report unknown byte totals.
