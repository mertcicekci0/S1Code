# Supported native verification commands

Native S1Code executes a bounded argv list after exact approval or explicit
session-wide consent. It does not interpolate a shell command from model text.
All checks execute trusted repository code; they are not isolated from your files
or network. Tool output, exit code and workspace revision are persisted.

Supported commands:

- `node --test`
- `python3 -m unittest`, optionally `-v` or `-q`
- `python3 -m unittest discover`, optionally `-s DIRECTORY`, `-v` or `-q`
- `python3 -m pytest`, optionally `-q`, `-v` or `--disable-warnings`
- `cargo test --offline` or `cargo check --offline`, optionally `--locked`,
  `--all-targets`, `--lib` or `--quiet`
- `npm --offline run test`, `build`, `lint` or `typecheck`

For npm, read `package.json` first. The exact named script must already exist and
be nonempty in an allowed manifest. The runtime validates it again before spawning;
changed workspace contents invalidate a pending approval. npm can run associated
pre/post lifecycle scripts too, so review those in an unfamiliar project.
Dependencies and tools must already be installed. Missing tools/dependencies are
reported; S1Code never silently installs them. `--offline` controls npm package
fetching, **not network access by the script itself**. Approved repository code may
read user configuration or contact services, just like tests in other languages.

Arbitrary script names, trailing npm arguments, package installation, shell strings
and path-changing flags are unsupported. pytest uses the environment's installed
module; test plugins/configuration still execute repository code. Its option list
intentionally excludes plugin imports, arbitrary path arguments and config overrides.

A zero exit code is recorded as a successful check, not proof of complete correctness.
Recognized empty unittest, Node, pytest and Cargo test summaries cannot satisfy
verification; build/typecheck/lint can provide non-test verification. Unknown output
formats do not imply a known test count. The generator must still justify completion
against the task and the available evidence.
