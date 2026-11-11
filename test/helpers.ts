/*
 * Shared test support. Not a test file: `node --test` runs everything under `test/`, and this
 * defines helpers only, so it runs and passes with nothing to report.
 */
import assert from "node:assert/strict";
import type { TestContext } from "node:test";
import { z } from "zod";
import type { ZodRawShape } from "zod";
import { TOKEN_ENV } from "../src/config/config.js";
import { Refused } from "../src/utils/errors.js";
import type { ApiBody } from "../src/models/api-body.js";

/**
 * A tool's outcome as a test reads it: the payload's fields are whatever the tool's output schema
 * names, which `expectOutput` checks, so a test reads them as loosely as a client would.
 */
export interface Outcome {
  payload: ApiBody;
  ok: boolean;
}

export const TOKEN = "glpat-test-token-1234";

/** Matches a refusal whose message matches `pattern`. */
export const isRefusal =
  (pattern: RegExp) =>
  (e: unknown): boolean =>
    e instanceof Refused && pattern.test(e.message);

/** Set the token variable for one test, or remove it when `value` is null, and restore it after. */
export function setToken(t: TestContext, value: string | null): void {
  const saved = process.env[TOKEN_ENV];
  if (value === null) delete process.env[TOKEN_ENV];
  else process.env[TOKEN_ENV] = value;
  t.after(() => {
    if (saved === undefined) delete process.env[TOKEN_ENV];
    else process.env[TOKEN_ENV] = saved;
  });
}

/** Set any environment variable for one test, or remove it when `value` is null, and restore it after. */
export function setEnv(t: TestContext, name: string, value: string | null): void {
  const saved = process.env[name];
  if (value === null) delete process.env[name];
  else process.env[name] = value;
  t.after(() => {
    if (saved === undefined) delete process.env[name];
    else process.env[name] = saved;
  });
}

export const withToken = (t: TestContext): void => setToken(t, TOKEN);
export const withoutToken = (t: TestContext): void => setToken(t, null);

/** One canned answer for `mockApi`. */
/** An answer `mockApi` hands back for one request. */
export interface Reply {
  status: number;
  body?: unknown;
  headers?: Record<string, string>;
}

/** A request `mockApi` saw. `body` is the JSON sent, decoded. */
export interface RecordedCall {
  url: string;
  method: string;
  headers: Record<string, string>;
  body: any;
}

export const reply = (status: number, body?: unknown, headers: Record<string, string> = {}): Reply => ({
  status,
  body,
  headers,
});

/**
 * Replace `fetch` with a queue of answers, one per request in order, and record every request.
 * A request beyond the queue fails the test, so an unexpected call cannot go unnoticed.
 */
export function mockApi(t: TestContext, ...answers: Reply[]): { calls: RecordedCall[] } {
  const calls: RecordedCall[] = [];
  const queue = [...answers];

  t.mock.method(globalThis, "fetch", async (url: string | URL, init: RequestInit = {}) => {
    calls.push({
      url: String(url),
      method: init.method ?? "GET",
      headers: (init.headers ?? {}) as Record<string, string>,
      body: init.body === undefined ? undefined : JSON.parse(init.body as string),
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
 */
export function expectOutput(TOOL: { name: string; outputSchema: ZodRawShape }, payload: Record<string, unknown>): void {
  const { summary, ...fields } = payload;
  assert.equal(typeof summary, "string", `${TOOL.name} has no summary`);
  const text = summary as string;
  assert.ok(text.length > 0 && text === text.trim(), `${TOOL.name} summary is blank or padded`);

  const parsed = z.object(TOOL.outputSchema).safeParse(fields);
  assert.ok(parsed.success, `${TOOL.name} payload breaks its outputSchema: ${parsed.success ? "" : parsed.error.message}`);
}
