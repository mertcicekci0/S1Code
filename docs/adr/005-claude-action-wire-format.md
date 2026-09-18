# Claude action wire format

The Claude adapter now sends a single action object schema with an explicit type
enum and nullable argument fields. The previous schema used an anyOf of ten object
variants, including no-argument variants. Simplifying this grammar is an experimental
compatibility change, not a confirmed explanation of model behavior or an efficiency
claim. No provider outputs are used to train or develop a decision model.

Only known unused top-level null arguments are removed at the provider boundary.
Nested patch before_hash nulls retain their create-file semantics. Unknown fields,
non-null arguments belonging to another action, missing required arguments and
invalid domain actions are rejected. All runtime policy, hash checks and approvals
remain unchanged. No action is inferred from prose, and the no-progress stop remains.

Offline tests cover wire conversion, rejection and a local HTTP streamed patch plus
real verification. Live coding acceptance must still be checked separately; passing
mock protocol tests cannot establish successful model action selection.
