# Privacy

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
versioned independently of the brand. There is no automatic migration; future moves
must validate version 1 records and preserve artifacts and hashes.

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
