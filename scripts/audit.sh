#!/bin/sh
set -eu
if command -v cargo-audit >/dev/null 2>&1; then
  cargo audit --deny warnings
elif [ -x target/release-tools/bin/cargo-audit ]; then
  target/release-tools/bin/cargo-audit audit --deny warnings --db target/advisory-db
else
  echo 'Install the audit tool: cargo install cargo-audit --locked --version 0.22.2 --root target/release-tools' >&2
  exit 1
fi
