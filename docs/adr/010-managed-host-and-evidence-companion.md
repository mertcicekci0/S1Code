# ADR 010: Keep managed hosts separate; expose bounded evidence ranking

Status: accepted, 2026-09-19.

Native S1Code owns the tool loop and recoverable context. An official host's
subscription login cannot be reused as a native generation credential. Users can
sign in through the unmodified official Claude Code CLI, which retains credential
ownership and execution responsibility.

Add explicit account-command forwarding and an opt-in MCP stdio companion. The
companion batches relevance questions about supplied excerpts using the existing
validated Jev adapter. It has a required process-level request allowance, exact
caching, cancellation, bounded inputs and no filesystem tools. It never approves
host actions, edits host transcripts or claims automatic compaction. Native Jev
selection and native reversible eviction remain independent configuration choices.

Do not copy an early-access function-hook implementation into a stable integration.
The inspected reference compaction adapter depends on `session.compact` replacement
semantics outside the standard PreCompact/PostCompact documented contract. Its
success does not grant a third-party runtime ownership of Claude's hidden turns.
A versioned first-party contract and separate interoperability validation would be
required before offering that replacement path.

Tradeoff: MCP ranking is host-invoked and adds no value if the host already has an
obvious next step. Its default instructions discourage ceremonial requests; it is
not a substitute for measured task evaluation. Restarting an MCP server resets the
request allowance, so provider-side monetary limits remain necessary.

No third-party source code is reused by this change.
