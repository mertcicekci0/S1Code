# Conversation view and explicit delegated follow-ups

The default runtime view previously exposed protocol activity and ended after one
upstream turn. A greeting with no test execution became blocked, which confused
conversation readiness with coding-task verification.

The default view now renders visible conversation, with F2 exposing the existing
inspectors. Exact action approvals remain visible. Codex accepts an explicit new
message after a completed turn using the same App Server process and thread;
each submission is a counted delegation. Awaiting input is separate from verified
completion. Resume observes the saved upstream turn before offering input, never
resubmitting a completed turn automatically. Native execution remains bounded tasks.

No hidden upstream context or inference controls are claimed. Wall-clock session
time includes human pauses. Process reuse is tested offline, not presented as a
measured inference-speed or cost improvement. Ratatui's pinned rendered-line-info
feature provides accurate wrapped-line scrolling without another layout engine.
