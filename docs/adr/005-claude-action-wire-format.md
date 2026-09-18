# Claude action wire format

The Claude adapter sends a single action object with an explicit type enum and
separate nullable argument objects named read, search, patch, run, rehydrate,
blocked and finish. Only the payload matching type may be non-null. No-argument
actions have all payloads null. This avoids a union of complete action objects and
prevents descriptive fields for one action from appearing in another action's
arguments. Descriptions belong in the visible message.

The initial flat nullable-field representation was too permissive: it allowed
reason/summary/path fields on action types that do not accept those arguments.
The final representation gives each payload its own strict schema. Canonical
domain actions remain unchanged. Runtime conversion rejects unknown fields,
multiple payloads, missing required arguments and invalid domain actions. Nested
patch before_hash nulls retain their create-file semantics. All policy, hash checks
and approvals remain unchanged. No action is inferred from prose.

Planning fingerprints include an explicit proposal-contract version, so a schema
fix can be tried on existing evidence. This does not reset request budgets or replay
completed tool actions. The no-progress stop remains enabled.

Offline tests cover wire conversion, rejection and a local HTTP streamed patch plus
real verification. Live coding acceptance is separate; mock protocol tests alone
cannot establish successful model action selection. No provider outputs are used
to train or develop a decision model. Restricted traces remain private.

Selection fallback now prefers an unexecuted, policy-allowed read/search/rehydration
candidate when distribution confidence is below the experimental threshold. This
is explicitly logged as deterministic evidence fallback, distinct from the model's
original choice. Uncertainty does not itself prohibit gathering evidence. Patch and
process approvals remain exact and unchanged; the model cannot grant permission.
