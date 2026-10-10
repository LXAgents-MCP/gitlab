import assert from "node:assert/strict";
import test from "node:test";
import { MAX_OUTPUT } from "../src/config.js";
import { describe, fence, renderPayload } from "../src/render.js";
import { firstLine, head, short, tail } from "../src/text.js";

test("a flat payload is a summary and one bullet per scalar, in order", () => {
  assert.equal(
    renderPayload("Created it.", { name: "demo", count: 3, draft: false }),
    "**Created it.**\n\n- name: demo\n- count: 3\n- draft: false",
  );
});

test("absent fields, empty lists and nested objects are left out of the text", () => {
  assert.equal(
    renderPayload("S", { a: undefined, b: null, c: [], d: { nested: 1 }, e: "kept" }),
    "**S**\n\n- e: kept",
  );
});

test("a list of scalars is one bullet, joined with commas", () => {
  assert.equal(renderPayload("S", { labels: ["bug", "ui"] }), "**S**\n\n- labels: bug, ui");
});

test("a list of rows is a heading and one bullet per row, scalars only", () => {
  assert.equal(
    renderPayload("2 branches.", {
      items: [
        { name: "main", sha: "abc1234", protected: true, nested: { x: 1 } },
        { name: "dev", sha: "def5678", protected: false, tags: ["a", "b"] },
      ],
    }),
    "**2 branches.**\n\n**items**\n- name: main · sha: abc1234 · protected: true\n- name: dev · sha: def5678 · protected: false · tags: a, b",
  );
});

test("a long or multi-line field of a row is an indented block under its bullet", () => {
  const long = "x".repeat(300);
  assert.equal(
    renderPayload("S", { comments: [{ author: "ada", body: "line one\nline two" }, { author: "bob", body: long }] }),
    `**S**\n\n**comments**\n- author: ada\n  body:\n  \`\`\`text\n  line one\n  line two\n  \`\`\`\n- author: bob\n  body:\n  \`\`\`text\n  ${long}\n  \`\`\``,
  );
});

test("a short field of a row stays on its bullet, and an over-long cell in describe is clipped only for scalars", () => {
  assert.equal(describe({ a: "short", b: ["x", "y"], c: 3 }), "a: short · b: x, y · c: 3");
  assert.equal(describe("z".repeat(250)), `${"z".repeat(200)}…`);
});

test("text is fenced as a block after the bullets", () => {
  assert.equal(
    renderPayload("Read it.", { path: "a.txt", text: "hello\n" }),
    "**Read it.**\n\n- path: a.txt\n\n```text\nhello\n```",
  );
});

test("a string with a line break, or too long for a bullet, is a block under its own name", () => {
  assert.equal(
    renderPayload("Read it.", { sha: "abc", message: "Subject\n\nBody line", note: "short" }),
    "**Read it.**\n\n- sha: abc\n- note: short\n\n**message**\n```text\nSubject\n\nBody line\n```",
  );
  const long = "z".repeat(250);
  assert.equal(renderPayload("S", { body: long }), `**S**\n\n**body**\n\`\`\`text\n${long}\n\`\`\``);
});

test("an empty text field adds no block", () => {
  assert.equal(renderPayload("S", { text: "" }), "**S**");
});

test("a fence is longer than any run of backticks inside the block", () => {
  assert.equal(fence("a ``` b"), "````text\na ``` b\n````");
  assert.equal(fence("plain"), "```text\nplain\n```");
});

test("describe renders a row on one line and a scalar as itself", () => {
  assert.equal(describe({ a: 1, b: "x", c: undefined, d: null, e: "" }), "a: 1 · b: x");
  assert.equal(describe("solo"), "solo");
  assert.equal(describe(null), "");
});

test("the rendered text is cut at the output ceiling", () => {
  const text = renderPayload("S", { text: "y".repeat(MAX_OUTPUT * 2) });
  assert.ok(text.length <= MAX_OUTPUT + 20);
  assert.match(text, /\.\.\. \(truncated\)$/);
});

test("short, firstLine, head and tail", () => {
  assert.equal(short("0123456789"), "0123456");
  assert.equal(short(undefined), "");
  assert.equal(firstLine("one\ntwo"), "one");
  assert.equal(firstLine(undefined), "");
  assert.deepEqual(head("abcdef", 3), ["abc\n... (truncated)", true]);
  assert.deepEqual(head("abc", 3), ["abc", false]);
  assert.deepEqual(tail("abcdef", 3), ["(truncated) ...\ndef", true]);
  assert.deepEqual(tail("abc", 3), ["abc", false]);
});

