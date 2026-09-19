import unittest

from service.checkout import charge


class CheckoutTests(unittest.TestCase):
    def test_duplicate_transaction_is_idempotent(self):
        transactions = {}
        self.assertEqual(charge(transactions, "tx-1", "alice", 20), 20)
        self.assertEqual(charge(transactions, "tx-2", "alice", 5), 25)
        self.assertEqual(charge(transactions, "tx-1", "alice", 999), 25)
        self.assertEqual(transactions["tx-1"], ("alice", 20))

    def test_customers_have_independent_totals(self):
        transactions = {}
        charge(transactions, "tx-1", "alice", 20)
        self.assertEqual(charge(transactions, "tx-2", "bob", 7), 7)

    def test_invalid_amount_does_not_reserve_transaction_id(self):
        transactions = {}
        with self.assertRaises(ValueError):
            charge(transactions, "retryable", "alice", 0)
        self.assertNotIn("retryable", transactions)
        self.assertEqual(charge(transactions, "retryable", "alice", 12), 12)


if __name__ == "__main__":
    unittest.main()
