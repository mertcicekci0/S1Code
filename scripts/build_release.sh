#!/bin/sh
# Remove local checkout and account paths from compiler diagnostics in distributed binaries.
set -eu
project_root=$(pwd -P)
export CARGO_ENCODED_RUSTFLAGS="$(printf '%s\037%s' "--remap-path-prefix=$HOME=/build-home" "--remap-path-prefix=$project_root=/src/s1code")"
unset RUSTFLAGS
cargo build --locked --release
