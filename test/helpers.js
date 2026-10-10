/*
 * Shared test support. Not a test file: `node --test` runs everything under `test/`, and this
 * defines helpers only, so it runs and passes with nothing to report.
 */
import assert from "node:assert/strict";
import { z } from "zod";
import { TOKEN_ENV } from "../src/config.js";
import { Refused } from "../src/errors.js";

export const TOKEN = "glpat-test-token-1234";

/** Matches a refusal whose message matches `pattern`. */
export const isRefusal = (pattern) => (e) => e instanceof Refused && pattern.test(e.message);

/** Set the token variable for one test, or remove it when `value` is null, and restore it after. */
export function setToken(t, value) {
  const saved = process.env[TOKEN_ENV];
  if (value === null) delete process.env[TOKEN_ENV];
  else process.env[TOKEN_ENV] = value;
  t.after(() => {
    if (saved === undefined) delete process.env[TOKEN_ENV];
    else process.env[TOKEN_ENV] = saved;
  });
}

export const withToken = (t) => setToken(t, TOKEN);
export const withoutToken = (t) => setToken(t, null);

/** One canned answer for `mockApi`. */
export const reply = (status, body, headers = {}) => ({ status, body, headers });

/**
 * Replace `fetch` with a queue of answers, one per request in order, and record every request.
 * A request beyond the queue fails the test, so an unexpected call cannot go unnoticed.
 *
 * @param {import("node:test").TestContext} t
 * @param {...{ status: number, body?: unknown, headers?: Record<string, string> }} answers
 * @returns {{ calls: { url: string, method: string, headers: Record<string, string>, body: any }[] }}
 */
export function mockApi(t, ...answers) {
  const calls = [];
  const queue = [...answers];

  t.mock.method(globalThis, "fetch", async (url, init = {}) => {
    calls.push({
      url: String(url),
      method: init.method ?? "GET",
      headers: init.headers ?? {},
      body: init.body === undefined ? undefined : JSON.parse(init.body),
    });
    const next = queue.shift();
    if (next === undefined) throw new Error(`unexpected request #${calls.length}: ${init.method ?? "GET"} ${url}`);

    const body = typeof next.body === "string" ? next.body : next.body === undefined ? null : JSON.stringify(next.body);
    return new Response(body, { status: next.status, headers: next.headers });
  });

  return { calls };
}

/**
 * A successful payload must satisfy the tool's own output schema - the same check a client makes
 * on `structuredContent` - and lead with a summary sentence.
 *
 * @param {{ name: string, outputSchema: Record<string, import("zod").ZodType> }} TOOL
 * @param {Record<string, unknown>} payload
 */
export function expectOutput(TOOL, payload) {
  const { summary, ...fields } = payload;
  assert.equal(typeof summary, "string", `${TOOL.name} has no summary`);
  assert.ok(summary.length > 0 && summary === summary.trim(), `${TOOL.name} summary is blank or padded`);

  const parsed = z.object(TOOL.outputSchema).safeParse(fields);
  assert.ok(parsed.success, `${TOOL.name} payload breaks its outputSchema: ${parsed.success ? "" : parsed.error.message}`);
}
