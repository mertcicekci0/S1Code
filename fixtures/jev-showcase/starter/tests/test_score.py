import unittest

from src.score import calculate_score


class ScoreTests(unittest.TestCase):
    def test_combo_multiplies_points(self):
        self.assertEqual(calculate_score(10, 3), 30)

    def test_combo_must_be_positive(self):
        with self.assertRaises(ValueError):
            calculate_score(10, 0)


if __name__ == "__main__":
    unittest.main()
