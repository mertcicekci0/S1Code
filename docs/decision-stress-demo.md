# Opus 5 harness decision stress demo

This demo compares official Claude Code with S1Code native mode using the same
generation model and an identical starting tree. It is designed to expose a harness
choice: several plausible definitions precede the production implementation.

Prepare both workspaces without starting an agent or making an API request:

```sh
python3 scripts/decision_stress.py
```

The script prints two paths, their common tree hash, and one prompt. Open separate
terminals in those paths. Start official Claude Code with Opus 5 in the `claude`
workspace. Start `s1code` in the other and select:

```text
/provider claude
/model opus
/decision jev
/jev typesafe
/eviction conservative
/permissions full-access
```

Paste the exact printed prompt into both. In S1Code, open **F2 → Decide** after the
literal search. The bounded frontier contains archived, compatibility, documentation,
example, migration, active-runtime, and production-import evidence. The eighth
candidate slot remains the generator escape route. Jev sees the matching line for
each candidate; it does not receive or execute the files themselves. Exact policy,
path checks, patch validation, execution, and test status remain deterministic.

After both agents stop, run this independently in each workspace:

```sh
python3 -m unittest discover -s tests -v
```

Review the diffs and confirm neither agent edited tests or decoy paths. Count extra
prompts as interventions. Different harnesses and authentication paths mean this is
a product demonstration, not an isolated model benchmark. Keep Jev traces, timing,
token, cost, and comparative service measurements private pending publication
clearance. Do not infer average superiority from a constructed single run.
