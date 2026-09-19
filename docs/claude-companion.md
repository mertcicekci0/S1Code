# Claude Code account login and Jev companion

Choose the native S1Code loop when you want S1Code to own action selection,
patches, verification and recoverable context. Choose the official Claude Code
terminal when you want its execution engine and account login. These are different
product paths; login does not turn a subscription into a native API credential.

## Official account login

Install the official Claude Code CLI using Anthropic's instructions, then:

```sh
s1code login claude
s1code account claude
s1code claude-code
```

The first command starts `claude auth login --claudeai`. Authentication completes
in Anthropic's flow. S1Code never imports the token. Account status returns only
connection state and a recognized authentication method; email and organization
identifiers are omitted. `s1code logout claude` explicitly logs out through the
official CLI. On the S1Code home screen, `/login claude` and `/account claude` expose
the same operations; `/claude-code` opens the official terminal.

The unmodified official client owns its tools, approvals, history, telemetry,
subscription limits and billing. Native `/provider claude` still requires an API
key. The terminal handoff does not copy saved native keys into the child environment.
An explicitly exported Anthropic API key can still select API billing in Claude
Code; use its own account controls to inspect the selected method.

## Optional Jev evidence ranking over MCP

This independently authored Rust companion exposes two tools:

- `rank_evidence`: evaluate 2–12 supplied excerpts in one batched Jev request and
  return IDs ordered by relevance. Each excerpt is limited to 4096 UTF-8 bytes;
  the task plus excerpts must fit 24000 bytes. Stable inputs use an exact cache.
- `decision_budget`: inspect remaining request allowance without contacting Jev.

It does **not** replace Claude Code's tool selection, remove its context, run local
commands or approve anything. The host decides whether to call it and how to use
its advisory result. It is useful for ambiguous evidence, not arithmetic, exact
facts, a single obvious candidate, or checking whether tests passed. Noul values
are not confidence or probabilities of correctness.

### Connect once per project

Save the separate TypeSafe key on macOS:

```sh
s1code auth set typesafe
```

Inside the project where you run Claude Code:

```sh
claude mcp add --scope local --transport stdio s1code -- s1code mcp --max-provider-requests 8
claude
```

If `s1code` is not on the CLI's PATH, use the installed binary's absolute path in
that configuration. The local scope is project-specific; use Claude Code's `/mcp`
to check connection and tool availability. Remove it with
`claude mcp remove --scope local s1code`. S1Code does not alter your MCP settings on
startup. Linux users must supply `TYPESAFE_API_KEY` through their secret manager or
inherited environment; avoid plaintext keys in MCP configuration and shell history.

The explicit allowance permits **up to eight billed TypeSafe request attempts per
server process**, including retries. A reconnect/restart renews it. It is neither a
persistent daily cap nor a dollar cap. Use provider spending controls for monetary
limits. Initialization, tool discovery and budget inspection make no API calls.

A task can ask for the tool naturally:

```text
Investigate the failing checkout test. If several implementations or diagnostics
look relevant, gather short excerpts and use s1code rank_evidence once to help
choose what to inspect next. Then make the smallest fix and run the tests.
Do not use ranking for obvious choices or as proof that the fix is correct.
```

### Data and ownership

Calling `rank_evidence` sends the supplied task and excerpts to the official TypeSafe
API with pinned `jev-1.13.0`, separately from Claude authentication and billing.
The server reads no repository files or Claude transcripts and stores no evidence
or provider outputs on disk. The host may retain tool inputs/results in its own
history. Known loaded secrets and common key prefixes are rejected before sending;
this is not a complete secret detector. Select excerpts deliberately.

Only one decision runs at a time. Cancellation interrupts network/backoff waits;
other decision requests while busy are rejected instead of queued. Closing the
transport cancels pending work. A failed API call returns a tool error, never an
invented ranking or silent provider switch. Results stay subject to the TypeSafe
service agreement; keep service measurements private pending clearance.

## Why this is not a compaction plugin

The inspected compaction reference uses an early-access function-hook contract to
replace a compaction result. Standard documented PreCompact/PostCompact hooks do
not provide that same replacement contract. We do not depend on experimental host
internals for the initial companion. Reversible eviction remains in native S1Code,
where the runtime owns canonical artifacts and can enforce its invariants.

This MCP companion can work alongside an eligible account login in the official
client because it calls TypeSafe directly and never uses the Claude token. It does
not make the delegated host a decision-first S1Code runtime, and no efficiency gain
is assumed from installing it.

References: [Claude authentication rules](https://code.claude.com/docs/en/legal-and-compliance),
[Claude MCP setup](https://code.claude.com/docs/en/mcp),
[MCP stdio](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports),
[MCP tools](https://modelcontextprotocol.io/specification/2025-06-18/server/tools).
