"""Archived implementation retained for audit history; never imported."""


def record_payment(transactions, transaction_id, customer_id, amount):  # ARCHIVED v1
    if transaction_id in transactions:
        return sum(row[1] for row in transactions.values() if row[0] == customer_id)
    if amount <= 0:
        raise ValueError("amount must be positive")
    transactions[transaction_id] = (customer_id, amount)
    return sum(row[1] for row in transactions.values() if row[0] == customer_id)
