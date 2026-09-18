# ADR 003: Task entry and explicit Claude ownership

Status: implemented, 2026-09-19.

Opening the executable previously asked for one line on stderr and immediately
entered an activity inspector. Users mistook the separate offline demonstration
for the normal product. The default entry is now a terminal task composer with
visible provider/authentication and decision ownership, slash commands, readiness
checks, and a return path after task completion. Each new prompt starts an independent
saved task; it does not imply inherited conversation context. Existing sessions are
resumed explicitly. Opening the composer starts no inference and no account login.

The existing native generator boundary now supports Claude's public Messages API
with structured streaming output. Claude only proposes complete alternatives;
S1Code still selects/revalidates/approves/executes. The same Jev and deterministic
policies work with either native generator. Provider selection is persisted with
sessions; pre-0.2 native sessions default to OpenAI. Resume never switches providers.
Native evaluation records the selected generation provider and uses it for all
matched decision policies. Claude usage fields retain their documented semantics.

A separate terminal handoff opens the installed official Claude Code executable
unchanged. It receives no task from S1Code and no bypass flags. The user signs in
and interacts through Anthropic's own UI. Claude Code owns credentials, permissions,
execution, telemetry and history. We neither capture its internal events nor call
this a S1Code/Jev run. This narrow handoff avoids inventing a third bridge protocol
or claiming unsupported subscription-token use. Native Claude remains API-key only.

The working product name is S1Code. Storage and writer-lock compatibility are
independent of the brand; the 0.1 store is reused safely instead of copied live.
No naming/trademark clearance is implied.
