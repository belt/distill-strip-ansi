import unittest

from distill_strip_ansi import contains_ansi, strip_bytes, strip_text


class TestBinding(unittest.TestCase):
    def test_strip_bytes(self):
        self.assertEqual(strip_bytes(b"a\x1b[31mb\x1b[0m"), b"ab")

    def test_strip_text(self):
        self.assertEqual(strip_text("caf\u00e9 \x1b[32mgreen\x1b[0m"), "caf\u00e9 green")

    def test_contains_ansi(self):
        self.assertFalse(contains_ansi(b"plain"))
        self.assertTrue(contains_ansi(b"\x1b]8;;https://example.com\x07link"))

    def test_empty_input(self):
        self.assertEqual(strip_bytes(b""), b"")
        self.assertFalse(contains_ansi(b""))


if __name__ == "__main__":
    unittest.main()