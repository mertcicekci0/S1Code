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
  built-in summary. The Swift presentation is explicitly scripted. Nerve instead
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
