# Jev decision showcase

This fixture demonstrates one bounded decision where the candidate order is
misleading. It is a functional inspection, not evidence of average quality, speed,
cost, or task success.

Prepare two byte-identical workspaces without making provider calls:

```sh
python3 scripts/jev_showcase.py
```

The command prints both workspace paths and the shared prompt. Open S1Code in each
directory. For the first session use `/decision rules`; for the second use
`/decision jev`. Keep the same generation provider, model, permissions, task, and
request limits. Paste the printed prompt yourself.

The literal search finds four plausible locations. The first is archived code; the
other matches expose historical documentation, the active source, and the test
import. Open **F2 → Decide** after the selection. It shows the selected action, the
policy, and the candidate-specific matching line that informed the choice. Jev is
called only for this genuine ambiguity; listing, validation, patch safety, process
exit status, and completion remain deterministic.

Run the same independent command in both workspaces afterward:

```sh
python3 -m unittest discover -s tests -v
```

Keep live Jev traces and comparative measurements private until the applicable
service terms permit publication. A public product recording may explain the
candidate-selection mechanism without publishing a provider benchmark. Never infer
an average advantage from this constructed one-run fixture.
