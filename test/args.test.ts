import assert from "node:assert/strict";
import test from "node:test";
import {
  maybeFlag,
  oneOf,
  optionalDate,
  optionalFlag,
  optionalInt,
  optionalText,
  rawText,
  requiredInt,
  requiredText,
  textList,
} from "../src/models/args.js";
import { isRefusal } from "./helpers.js";

test("optionalText trims, drops blanks and refuses a non-string", () => {
  assert.equal(optionalText(undefined, "x"), undefined);
  assert.equal(optionalText(null, "x"), undefined);
  assert.equal(optionalText("   ", "x"), undefined);
  assert.equal(optionalText("  hi ", "x"), "hi");
  assert.throws(() => optionalText(5, "x"), isRefusal(/`x` must be a string, got number\./));
});

test("requiredText refuses a missing or blank value", () => {
  assert.equal(requiredText(" a ", "x"), "a");
  for (const value of [undefined, null, "", "  "]) {
    assert.throws(() => requiredText(value, "x"), isRefusal(/`x` is required and must be a non-empty string\./));
  }
});

test("rawText keeps the text exactly and checks only that it is a string", () => {
  assert.equal(rawText("  keep\n", "x"), "  keep\n");
  assert.equal(rawText(undefined, "x"), undefined);
  assert.throws(() => rawText(undefined, "x", { required: true }), isRefusal(/`x` is required\./));
  assert.throws(() => rawText("  ", "x", { required: true }), isRefusal(/must not be blank/));
  assert.equal(rawText("", "x", { required: true, allowEmpty: true }), "");
  assert.throws(() => rawText(3, "x"), isRefusal(/`x` must be a string, got number\./));
});

test("optionalFlag falls back when absent and refuses a non-boolean", () => {
  assert.equal(optionalFlag(undefined, "x", true), true);
  assert.equal(optionalFlag(null, "x", false), false);
  assert.equal(optionalFlag(false, "x", true), false);
  assert.throws(() => optionalFlag("yes", "x", true), isRefusal(/`x` must be true or false, got string\./));
});

test("optionalInt and requiredInt take whole numbers inside the range", () => {
  assert.equal(optionalInt(undefined, "n"), undefined);
  assert.equal(optionalInt(5, "n", { min: 1, max: 10 }), 5);
  assert.throws(() => optionalInt(1.5, "n"), isRefusal(/`n` must be a whole number, got 1\.5\./));
  assert.throws(() => optionalInt("3", "n"), isRefusal(/must be a whole number, got "3"/));
  assert.throws(() => optionalInt(0, "n"), isRefusal(/`n` must be between 1 and/));
  assert.throws(() => optionalInt(11, "n", { max: 10 }), isRefusal(/`n` must be between 1 and 10, got 11\./));
  assert.equal(requiredInt(7, "n"), 7);
  assert.throws(() => requiredInt(undefined, "n"), isRefusal(/`n` is required and must be a whole number\./));
});

test("oneOf returns the fallback when absent and refuses a word outside the set", () => {
  assert.equal(oneOf(undefined, "m", ["a", "b"], "a"), "a");
  assert.equal(oneOf(" b ", "m", ["a", "b"]), "b");
  assert.equal(oneOf(undefined, "m", ["a", "b"]), undefined);
  assert.throws(() => oneOf("c", "m", ["a", "b"]), isRefusal(/`m` must be one of a, b, got 'c'\./));
});

test("textList trims, drops blanks and duplicates, and enforces its bounds", () => {
  assert.deepEqual(textList([" a ", "b", "a", "  "], "l"), ["a", "b"]);
  assert.equal(textList(undefined, "l"), undefined);
  assert.throws(() => textList(undefined, "l", { required: true }), isRefusal(/`l` is required/));
  assert.throws(() => textList(["  "], "l", { required: true }), isRefusal(/`l` is required/));
  assert.throws(() => textList("a", "l"), isRefusal(/`l` must be a list of strings\./));
  assert.throws(() => textList([1], "l"), isRefusal(/`l` must be a list of strings\./));
  assert.throws(() => textList(["a", "b", "c"], "l", { max: 2 }), isRefusal(/at most 2 values, got 3/));
});

test("maybeFlag keeps not-given apart from false", () => {
  assert.equal(maybeFlag(undefined, "x"), undefined);
  assert.equal(maybeFlag(null, "x"), undefined);
  assert.equal(maybeFlag(false, "x"), false);
  assert.equal(maybeFlag(true, "x"), true);
  assert.throws(() => maybeFlag("no", "x"), isRefusal(/`x` must be true or false, got string\./));
});

test("optionalDate takes an ISO 8601 date or timestamp and refuses anything else", () => {
  assert.equal(optionalDate(undefined, "since"), undefined);
  assert.equal(optionalDate("  ", "since"), undefined);
  assert.equal(optionalDate("2026-10-01", "since"), "2026-10-01");
  assert.equal(optionalDate(" 2026-10-01T12:00:00Z ", "since"), "2026-10-01T12:00:00Z");
  for (const bad of ["yesterday", "10/01/2026", "2026-13-45", "2026-1-1"]) {
    assert.throws(() => optionalDate(bad, "since"), isRefusal(/`since` must be an ISO 8601 date/), bad);
  }
  assert.throws(() => optionalDate(5, "since"), isRefusal(/`since` must be a string/));
});
