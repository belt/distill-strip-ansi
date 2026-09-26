import assert from "node:assert/strict";
import { test } from "node:test";
import { containsAnsi, strip } from "./index.js";

test("strips ANSI bytes", () => {
  assert.equal(strip(Buffer.from("a\x1b[31mb\x1b[0m")).toString(), "ab");
});

test("detects ANSI sequences", () => {
  assert.equal(containsAnsi(Buffer.from("plain")), false);
  assert.equal(containsAnsi(Buffer.from("\x1b[31mred")), true);
});

test("accepts empty input", () => {
  assert.deepEqual(strip(Buffer.alloc(0)), Buffer.alloc(0));
  assert.equal(containsAnsi(Buffer.alloc(0)), false);
});