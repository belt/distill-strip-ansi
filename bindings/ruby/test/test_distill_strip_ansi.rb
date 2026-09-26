require "minitest/autorun"
require_relative "../lib/distill_strip_ansi"

class DistillStripAnsiTest < Minitest::Test
  def test_strips_ansi_bytes
    assert_equal "ab", DistillStripAnsi.strip("a\e[31mb\e[0m")
  end

  def test_detects_ansi
    refute DistillStripAnsi.contains_ansi?("plain")
    assert DistillStripAnsi.contains_ansi?("\e]8;;https://example.com\a")
  end

  def test_empty_input
    assert_equal "", DistillStripAnsi.strip("")
    refute DistillStripAnsi.contains_ansi?("")
  end
end