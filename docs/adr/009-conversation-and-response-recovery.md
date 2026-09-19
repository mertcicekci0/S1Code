# ADR 009: Separate conversation from verified work; retry responses, not tools

Status: accepted, 2026-09-19.

The native loop previously forced discovery before responding to every input and
only offered coding completion after a successful check. A greeting could therefore
lead to unnecessary reads and tests. A mid-stream provider error also lost its type
and request ID behind a generic diagnostic, making the real cause unknowable later.

Exact short greetings now receive a local, recorded reply without provider or tool
calls. This is a narrow deterministic shortcut, not an intent model; a greeting
followed by instructions continues through the normal loop. `answer` is a separate
proposal action for questions or clarification. A valid text-only Claude `end_turn`
can become an answer, never an executable action. Answers yield `awaiting_input`;
only a coding `finish` with current verification yields `completed`. The terminal
retains answer metrics in the inspector without displaying a failed-check summary.

Claude failures carry selected sanitized fields and known usage. Authentication,
validation, refusals, invalid proposals and unknown error types stop. Documented
transient failures and interrupted streams permit at most two response retries with
backoff and jitter, within the already authorized generation/request caps. HTTP 429
without a retry hint stops because it may indicate a spend cap. Retry-After is
respected; delays over 60 seconds stop instead of retrying early. Cancellation
interrupts waits. The one smaller-action recovery for output truncation remains
separately bounded within the same total caps.

An adapter invocation makes one request. Partial output is discarded; no proposed
action is accepted until the stream completes and validates. Retrying a response
never replays a previously completed tool, resubmits an approved patch, resets
counters or changes providers. Initial output usage on an interrupted stream is
unknown as a final total. Tests use local HTTP fixtures plus actual disposable
patches and test processes; these checks do not establish live availability.
