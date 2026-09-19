"""Compatibility sketch for an abandoned client; not packaged or imported."""


def record_payment(entries, key, owner, value):  # UNUSED compatibility shim
    entries.setdefault(key, (owner, value))
    return value
