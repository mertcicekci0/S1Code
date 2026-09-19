# Payment migration notes

The old `record_payment` prototype in `archive/ledger_v1.py` is retained for audit
history. The checkout service imports its live implementation from `runtime.ledger`.
