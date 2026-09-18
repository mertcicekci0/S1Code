import unittest
from parser import parse_count


class ParserTests(unittest.TestCase):
    def test_whole_integer(self):
        self.assertEqual(parse_count(" 42 "), 42)

    def test_signed(self):
        self.assertEqual(parse_count("-12"), -12)

    def test_blank(self):
        self.assertEqual(parse_count("  "), 0)

    def test_invalid(self):
        with self.assertRaises(ValueError):
            parse_count("12x")
