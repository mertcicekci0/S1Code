# Contributing

Use Rust 1.94.0 and Cargo.lock. Keep provider formats inside adapters and terminal
code outside the engine. Run `scripts/check.sh` before proposing a change. Default
tests need no credentials or external network; protocol mocks use loopback sockets.
Live tests are explicitly ignored and require consent/credentials as documented.

Unless explicitly stated otherwise, contributions intentionally submitted for
inclusion are licensed under Apache-2.0, the project license. No contributor
assignment or invented copyright ownership is required. Retain third-party notices
and document any intentionally reused code and modifications separately. Do not
claim independently authored work for copied material.

Keep restricted measurements, tokens and personal trace data out of Git. Avoid
performance claims without reproducible evidence and provider clearance. Match
native policy settings, generation model, budgets, tools and fixture trees when
comparing decision policies; vary context eviction separately.

Use focused commits under your configured human Git identity. This project does not
require generated-by or AI co-author trailers. Do not commit changes to fixture
answers merely to make agent evaluation pass. Add meaningful invariant tests for
new execution paths, especially recovery and permission changes.
