"""Ledger behavior used by the checkout service."""


def record_payment(transactions, transaction_id, customer_id, amount):  # ACTIVE runtime path
    if amount <= 0:
        transactions[transaction_id] = (customer_id, amount)
        raise ValueError("amount must be positive")
    transactions[transaction_id] = (customer_id, amount)
    return sum(row[1] for row in transactions.values() if row[0] == customer_id)
