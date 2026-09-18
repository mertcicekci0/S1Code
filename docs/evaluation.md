# Evaluation methodology

`nerve eval --suite fixtures/core` is offline evaluator validation. It copies only
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
nerve eval --suite fixtures/core --live --approve-fixture-execution \
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
only `--eviction`. `nerve context-demo` verifies real eviction/rehydration mechanics
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
