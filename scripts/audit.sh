#!/bin/sh
set -eu
if ! command -v cargo-audit >/dev/null 2>&1; then
  echo 'Install the audit tool explicitly: cargo install cargo-audit --locked' >&2
  exit 1
fi
cargo audit --deny warnings
