# Architecture

Nerve owns the native observe → propose → filter → select → revalidate → execute
→ record → verify loop. Model proposals are data. Deterministic code enforces
permissions, hashes, bounds, and completion conditions. Decisions choose concrete
actions, with generation or blocked outcomes as escape routes.

The library separates domain, engine, decisions, generation, tools, context,
sessions, and policy. The terminal and JSONL interface consume the same events.
One writer owns a workspace. Storage uses append-only events, hashed artifacts,
and atomic checkpoint replacement. It is not an operating-system sandbox.

Codex mode delegates to an official local App Server. Its tool execution and
internal model turns belong to that runtime; Nerve never reexecutes upstream tools.
Delegation counts and unknown internal usage remain separate from native metrics.

