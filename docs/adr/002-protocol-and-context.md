# 002: Versioned local protocol and conservative context policies

The official installed CLI reports 0.153.3. Its generated App Server schema still
includes `untrusted`, while the executable rejects that setting at startup. The
current web documentation uses a different spelling in some examples. A real
handshake probe established that the schema's granular policy is accepted. A thread-start probe further established that this
policy requires the documented `experimentalApi` capability; the handshake opts
in explicitly for that policy. Unknown experimental requests still fail closed. Nerve
sets every granular approval gate to true, keeps read-only sandboxing with network
disabled, and forces the user approval reviewer. It verifies returned permissions
before delegating. This is a process-local override, not an account/config change.

Native and bridge controls cannot be identical. Native path exclusions are rejected
in bridge mode, whose internal tools/context are owned by the official runtime.
No fallback to weaker sandbox settings is permitted. Unsupported client requests
receive errors or empty permission grants; Nerve does not execute them locally.

Context storage groups each native call with its result. Compaction operates on
whole groups and retains dependency closure. Actual output excerpts and diagnostic
lines are available to Jev, rather than asking it to judge unseen content by size.
The independent eviction policy defaults to deterministic conservation. Jev scoring
failures require an explicit fallback setting; retained pinned overflow stops.
