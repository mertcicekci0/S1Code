from runtime.ledger import record_payment  # production import selected by checkout


def charge(transactions, transaction_id, customer_id, amount):
    return record_payment(transactions, transaction_id, customer_id, amount)
