#!/bin/sh
set -eu
python3 scripts/test_try_live.py
python3 scripts/test_snake_compare.py
python3 scripts/release_check.py
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
python3 scripts/dependency_licenses.py
