# 001: One library, two execution ownership models

Accepted. One package with a reusable library and a thin terminal binary minimizes
integration overhead. Native actions use fully specified typed arguments and
workspace fingerprints. Proposed file changes use hash-checked full UTF-8 file
replacements, with new-file support; rename, deletion, binary and unified-diff
input are deliberately unsupported in v0. Review uses generated unified diffs.
This reduces patch ambiguity and allows validation of every file before mutation.

Storage is independently versioned. A future rename may move the configured data
directory after checking its format version; it must not reinterpret old records.
There is no automatic migration or implicit cross-provider fallback.

