# 006: Bounded retention state and stable generation prefixes

S1Code's canonical artifacts and the model's working set are separate. Removing
active evidence does not delete captured bytes or require a command to be rerun.
This is reversible eviction, not lossless understanding.

The initial implementation reread and serialized every active artifact after each
eviction. It also sent all eligible excerpts in every retention-question batch.
The replacement computes exact JSON string-size deltas from one verified read per
active artifact, plans whole dependency/call groups, and applies the plan only if
the final working set fits. Tiny evidence is retained when a placeholder would
grow the prompt. Pinned constraints, diagnostics and recent dependency closure
remain protected. An overflow leaves the previous working set intact.

Independent Jev retention questions still share an unchanged session snapshot,
but each bounded batch receives only the excerpts it evaluates, plus the task and
workspace revision. This removes duplicated evidence from the request bodies.
Excerpts include diagnostic lines and capture/dependency metadata. Batch size is
eight; both configurable conservative token-estimate limits still apply. A task
that is itself too large fails explicitly. No concurrency or extra retry loop was
added: cancellation and the existing total provider-request budget remain binding.

Claude generation now sends the stable task/constraints and each captured evidence
item before dynamic workspace, freshness, verification and candidate metadata. One
documented five-minute cache breakpoint marks the stable prefix. Derived historical
flags move to the dynamic suffix; captured revisions and exact content remain in
the stable evidence. Appending evidence preserves earlier text; eviction changes
the affected prefix and is already recorded as an invalidation. OpenAI request
format is unchanged. The visible generation message requests two short sentences
instead of repeating structured candidate lists and hashes.

Cache writes can cost more than ordinary input, minimum cacheable lengths apply,
and a breakpoint is not a cache hit. Returned read/write usage is the evidence.
These changes have offline invariant, protocol and mechanics checks. Their comparative effect
on live task success, billable tokens, cost and latency is not established. The
probe is neither a service benchmark nor an external-project comparison. This
design was independently authored; no reference implementation was copied.
