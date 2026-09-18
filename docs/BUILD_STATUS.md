# Build status

Initial audit: empty workspace, no existing license/history to replace. A separate
sibling project is outside this project's scope and remains untouched.

Implemented: Apache-2.0 package foundation; pinned Rust 1.94.0; independent design.
In progress: native engine, tools, streaming generation, session persistence.
Pending: decisions, context eviction, bridge, terminal, evaluation, release checks.
Live validation: no OpenAI or TypeSafe API keys available in the environment.
No provider calls, performance claims, hosted CI, releases, or publication performed.


Milestone 2: native loop and real fixture verified offline. Six tests passed: complete
parser task with three approvals, restart, file/path guards, stale selection, whole
patch validation, fragmented UTF-8 SSE, reversible eviction/pinned overflow and
property-based command denial. HTTP adapter contract authored; live inference is
unverified. Terminal currently uses a basic approval prompt; full TUI pending.

Milestones 3–5: real Jev typed HTTP, retries/cache, candidate selection, constrained
generative comparison policy, excerpt-based eviction, context-demo, official
App Server bridge with managed login/status/logout, conservative granular
approvals and duplicate request handling, and four-view terminal are implemented.
18 offline tests pass. Five development fixtures and one held-out fixture pass
offline evaluator validation (not agent performance). Context demo performed five
real reads, evicted two artifacts, and rehydrated exact bytes without rerunning.
Official CLI managed account status/handshake passed; billed inference remains
unverified because provider API keys are unavailable. Release hardening/docs
and full protocol creation/resume checks continue.
