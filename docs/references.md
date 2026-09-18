# References and provenance

Reviewed 2026-09-18. No reference-project source was copied, translated, or adapted.
API field names implement public wire contracts. Newly authored code is Apache-2.0.

- [TypeSafe API](https://docs.typesafe.ai/api), [index](https://docs.typesafe.ai/llms.txt),
  [models](https://docs.typesafe.ai/models), [confidence](https://docs.typesafe.ai/confidence),
  [jaggedness](https://docs.typesafe.ai/model-jaggedness/jev-1.13): typed questions and
  answers, version pinning, distinct probability/confidence, two context limits.
  Default model: jev-1.13.0. Limits are configurable, conservatively estimated.
- [TypeSafe MCA](https://typesafe.ai/legal/mca), updated August 27, 2026: service
  license is separate from source license. Section 2.3 restricts publication of
  service benchmarks and use of service/outputs for imitation or competing
  development. Measurements stay private pending documented clearance; no training.
- [fast-jev-compaction](https://github.com/tamaratran/fast-jev-compaction), commit
  `e3f262a7f4d42bd8dd32ced30d26176f7cb545b0`, MIT. Consulted `src/state.ts`,
  `src/request.ts`, `src/compact.ts`, `src/messages.ts`, `hooks/fast-jev.ts`,
  `demo/JevDemo/main.swift`, README and LICENSE. The library groups call/results,
  batches independent questions, estimates tokens and throws failures to callers.
  Its fitted state often omits actual output content. The hook can fall back to a
  built-in summary. The Swift presentation is explicitly scripted. S1Code instead
  retains canonical bytes and presents actual excerpts to eviction decisions.
  No code reuse or modifications of that project.
- [Harness article](https://www.langchain.com/blog/building-a-harness-with-jev):
  considered bounded decision integration as a design question, not evidence for
  performance claims. The source was fetched directly after the browser could not
  retrieve it. No article text or implementation reused.
- [Codex App Server](https://developers.openai.com/codex/app-server/) and
  [authentication](https://developers.openai.com/codex/auth/): official local
  stdio transport, managed account operations, threads, turns and approvals.
  Installed `codex-cli 0.153.3`; consulted its `app-server --help` and generated
  JSON schemas. No credentials read. Version compatibility is checked explicitly.
- [Responses streaming](https://developers.openai.com/api/docs/guides/streaming-responses)
  and [structured output](https://developers.openai.com/api/docs/guides/structured-outputs):
  SSE text deltas, terminal events, usage and schema-constrained proposals.
- [Claude legal/authentication boundaries](https://code.claude.com/docs/en/legal-and-compliance):
  no Claude subscription integration implemented.
- [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0): unmodified license text.

Reference snapshots and local protocol schemas were inspected outside the project.
Provider documentation can change. Release claims require fresh agreement review
and explicit live checks, not just mocked contract tests.

2026-09-19 gateway follow-up: [OpenRouter OpenAPI](https://openrouter.ai/openapi.json),
`DecisionsRequest`, `DecisionsResponse`, Choice/Noul/Score schemas and
`/api/alpha/decisions`; [Jev model listing](https://openrouter.ai/typesafe/jev-1.13/)
and public `/api/v1/models/typesafe/jev-1.13/endpoints` metadata. The served build
was `typesafe/jev-1.13-20260917`. No third-party adapter implementation was copied.
Local fixtures, not paid inference, validated this integration during development.

2026-09-19 Claude and entry-screen follow-up: official
[Messages API](https://platform.claude.com/docs/en/api/messages/create),
[stream event contract](https://platform.claude.com/docs/en/build-with-claude/streaming),
[structured output contract](https://platform.claude.com/docs/en/build-with-claude/structured-outputs),
[model IDs](https://platform.claude.com/docs/en/models/overview),
[Claude Code CLI reference](https://code.claude.com/docs/en/cli-reference), and
[authentication and credential rules](https://code.claude.com/docs/en/legal-and-compliance).
Reviewed `message_start`, content blocks/deltas/stops, `message_delta`, `message_stop`,
usage, refusals and `output_config.format`. Implemented independently; no upstream
implementation copied. Official local `claude --version`: 2.1.266. Rechecked official
Codex App Server managed `account/login/start`/`account/read` documentation. Native
Claude and handoff process tests use clearly identified local fixtures, not billed
provider inference or an authenticated Claude Code coding task.

2026-09-19 HTTP failure follow-up: rechecked official
[Claude API errors](https://platform.claude.com/docs/en/api/errors) and structured
outputs documentation above. Error fields are `error.type`, `error.message`,
`request_id`. HTTP 400 alone cannot distinguish request validation from configured
spend limits. Only bounded, redacted selected fields are displayed; no raw headers.
