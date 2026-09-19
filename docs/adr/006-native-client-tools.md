# Native client-tool proposals and bounded generation

Claude native generation now uses documented client tool definitions and streamed
`tool_use` arguments. The prior nullable-payload JSON envelope encouraged competing
implementations and wasted output. Each tool schema describes only its own arguments.
The provider does not execute tools: the adapter converts a complete validated
response into domain candidates, and the existing engine owns selection, approval,
workspace revalidation, mutation and verification. Unknown tools and incomplete
arguments never execute. There is no SDK-owned autonomous loop.

Default instructions request one concrete next action. Alternatives remain useful
only when a genuine choice exists. A single admissible concrete action needs no
classifier call. Empty repositories go directly from bounded listing to generation;
searching arbitrary words from the task in an empty repository has no value.
Rejected candidates produce explicit validation feedback for the next generation.
Jev still selects among ambiguous candidates and scores eligible evidence for
reversible eviction; it does not invent missing command or patch arguments.

The native output ceiling is independently configurable (default 16,384 tokens).
For Opus 5 and Sonnet 5, Claude requests explicitly use medium effort by default;
other model families retain their provider default unless effort is configured.
Effort is a quality/cost tradeoff, not a correctness guarantee. Output includes
thinking on models with default thinking; when reported, reasoning tokens are
recorded as a subset of output, never added twice. Thinking text is not displayed
or stored. Request/generation caps still bound recovery attempts.

Provider snapshots remain explicit evidence documents, rather than fabricated
assistant tool calls. This preserves the distinction between proposed, rejected and
actually executed actions, and permits reversible eviction without orphaning wire
messages. No hidden reasoning continuity is claimed for these snapshots.

Offline HTTP fixtures cover fragmented inputs, unknown tool names, missing terminal
events, truncation and genuine local patch/test execution. Those checks establish
protocol and execution invariants, not live model coding success or efficiency.
