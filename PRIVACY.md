# Privacy

The installed binary's `auth set PROVIDER` command (or `/auth PROVIDER` on the
home screen) stores an API key in macOS login Keychain through the system Security
framework. Input is hidden and never passed in process arguments, exported into
the environment, or written to the project. Accounts are `openai`, `claude`,
`typesafe` and `openrouter` under `s1code.credentials.v1`; the three existing launcher
accounts remain compatible. `auth remove PROVIDER` deletes only that local entry,
not a key in the provider account. Explicit environment variables override saved
keys. Linux users currently provide keys through their environment or secret manager.

No S1Code telemetry is enabled. Native mode sends the task and selected active
repository/tool evidence to the configured generation provider. Selecting Jev also
sends focused decision evidence to TypeSafe. No silent cross-provider fallback is
implemented. Provider processing/retention is governed by their agreements;
`store:false` in Responses is not a general zero-retention guarantee.

Choosing `--jev-provider openrouter` sends that decision evidence and the separately
configured OpenRouter key to OpenRouter's Decisions API, restricted to the TypeSafe
provider with fallback disabled. This is an explicit gateway choice; the key is not
sent to the direct TypeSafe endpoint or child tools. OpenRouter is an additional
processor under its own terms. The same private-result/export restrictions apply.

Codex bridge mode sends the task to the official local runtime, which manages its
own authentication and provider traffic. S1Code disables analytics in its child
invocation, not in the user's account settings. S1Code does not read credential files.

Sessions contain task text, actions, patches, events, verification, local workspace
paths and content-addressed captured evidence. The data directory is private to the
user on Unix (directory mode 0700); data is not encrypted. The default is the OS local
data directory for `s1code`; `S1CODE_HOME` or `--home` overrides it. Storage schema is
versioned independently of the brand. Existing legacy stores are reused in place
when the new store is absent; NERVE_HOME remains a compatibility override. No
automatic move or merge occurs. See architecture.md for selection precedence.

Known environment secrets and personal paths are removed from event display/export.
Raw patch backups and canonical source evidence can contain incidental secrets not
recognized by the redactor. Do not put keys in source or task text. Review all exports
before sharing. Redaction is best effort, not a guarantee of secret detection.

`s1code export ID /path/to/trace.jsonl` exports redacted events, not raw artifacts.
Jev traces cannot be exported through this command pending documented clearance.
`s1code replay TRACE` labels every emitted record as replay. Native simulated demo
sessions retain their simulation label. Local eval results are private and gitignored;
a custom output outside the checkout remains your responsibility.

`s1code delete ID` removes that local session and its artifacts. Backups, exported
traces and provider-side records are separate. Native workspace locks contain no
source. Deleting a S1Code session does not delete an official Codex thread or log out.

Native `--provider claude` sends focused task/context evidence to Anthropic's public
Messages API under the API key owner's agreement. Provider changes are explicit;
Claude failures never send evidence to OpenAI. Provider secrets are excluded from
native tools. The home screen sends no task until the user submits one.

The optional official Claude Code handoff delegates the terminal itself. Its own
client settings govern its traffic and telemetry; S1Code does not intercept them.
S1Code does not capture that application's session or credentials.

The source live launcher can remember provider keys in macOS Keychain, service
`s1code.credentials.v1`, accounts `claude`, `typesafe`, `openrouter`. Missing keys
entered on macOS are saved there for reuse; `--no-keychain` opts out. Use
`python3 scripts/try_live.py --forget-keys` to delete those records. No plaintext key
file is created. Python and the selected provider child hold credentials in memory;
macOS access controls apply to the interpreter. The standalone binary reads these same records through the system security utility
when an environment key is absent. It captures secrets privately, retains them in
provider memory for requests/redaction, and does not export them to child environments. Linux launcher input remains temporary.

Native Claude requests use the provider's documented five-minute ephemeral prompt
cache for stable task/evidence prefixes. Cache retention is provider-side and
separate from local artifacts and session deletion. Cache writes/reads are recorded
when returned. This is not a zero-retention promise.
