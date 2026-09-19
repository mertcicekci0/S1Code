def record_payment(row):  # ONE-OFF completed migration, not runtime code
    return {**row, "amount": int(row["amount"])}
