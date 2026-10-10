import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { request } from "node:http";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { MAX_BODY_BYTES } from "../src/config.js";
import { httpSettings, startHttpServer } from "../src/http.js";
import { SERVER_NAME, VERSION } from "../src/version.js";
import { mockApi, reply, withToken } from "./helpers.js";
import { TOOL_NAMES } from "./surface.js";

/*
 * The HTTP transport over real sockets on loopback. The client here is `node:http`, not `fetch`,
 * because `fetch` is what the tests replace to stand in for GitLab; the two must not be the same.
 * This process serves HTTP for the whole file, which is why no test here reads the environment's
 * token.
 */

const ENTRY = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "index.js");
const JSON_RPC = { "Content-Type": "application/json", Accept: "application/json, text/event-stream" };

/** A running server on a free loopback port, closed when the test ends. */
async function serve(t) {
  const running = await startHttpServer({ port: 0, host: "127.0.0.1" });
  t.after(() => running.close());
  return `http://127.0.0.1:${running.port}`;
}

/** One request, answered with its status, headers and parsed body. */
function send(base, { method = "POST", path = "/mcp", headers = {}, body } = {}) {
  return new Promise((resolve, reject) => {
    const req = request(new URL(path, base), { method, headers }, (res) => {
      const chunks = [];
      res.on("data", (chunk) => chunks.push(chunk));
      res.on("end", () => {
        const text = Buffer.concat(chunks).toString("utf8");
        let json;
        try {
          json = JSON.parse(text);
        } catch {
          json = undefined;
        }
        resolve({ status: res.statusCode, headers: res.headers, text, json });
      });
    });
    req.on("error", reject);
    req.end(body);
  });
}

/** A JSON-RPC call to `/mcp` carrying a bearer token. */
function rpc(base, token, method, params, id = 1) {
  return send(base, {
    headers: { ...JSON_RPC, ...(token === null ? {} : { Authorization: `Bearer ${token}` }) },
    body: JSON.stringify({ jsonrpc: "2.0", id, method, params }),
  });
}

/* ------------------------------------------------------------------ the routes */

test("GET /healthz is open and says nothing but who is there", async (t) => {
  const base = await serve(t);

  const { status, json } = await send(base, { method: "GET", path: "/healthz" });

  assert.equal(status, 200);
  assert.deepEqual(json, { status: "ok", server: SERVER_NAME, version: VERSION });
});

test("the exemption is exactly GET /healthz", async (t) => {
  const base = await serve(t);

  for (const probe of [
    { method: "HEAD", path: "/healthz" },
    { method: "GET", path: "/healthz/" },
    { method: "POST", path: "/healthz" },
    { method: "GET", path: "/healthz2" },
  ]) {
    assert.equal((await send(base, probe)).status, 401, `${probe.method} ${probe.path}`);
  }
});

test("a request without a usable token is a 401 in the JSON-RPC envelope, before anything is read", async (t) => {
  const base = await serve(t);
  const refusal = { jsonrpc: "2.0", id: null };

  const missing = await send(base, { headers: JSON_RPC, body: "this is not json" });
  assert.equal(missing.status, 401, "the token is checked before the body is parsed");
  assert.equal(missing.headers["www-authenticate"], 'Bearer realm="mcp"');
  assert.deepEqual(missing.json, {
    ...refusal,
    error: { code: -32001, message: "Unauthorized: send your GitLab token as Authorization: Bearer <token>" },
  });

  for (const authorization of ["Basic dXNlcjpwYXNz", "Bearer", "Bearer a b", "Token abc"]) {
    const wrong = await send(base, { headers: { ...JSON_RPC, Authorization: authorization }, body: "{}" });
    assert.equal(wrong.status, 401, authorization);
    assert.equal(wrong.headers["www-authenticate"], 'Bearer realm="mcp"', authorization);
  }

  for (const token of ["a".repeat(513), "tokén"]) {
    const invalid = await send(base, { headers: { ...JSON_RPC, Authorization: `Bearer ${token}` }, body: "{}" });
    assert.equal(invalid.status, 401);
    assert.equal(invalid.headers["www-authenticate"], 'Bearer realm="mcp", error="invalid_token"');
    assert.deepEqual(invalid.json, { ...refusal, error: { code: -32001, message: "Unauthorized: the token is not valid" } });
    assert.ok(!invalid.text.includes(token), "the answer never echoes the token");
  }
});

test("a caller without a token cannot learn which routes exist", async (t) => {
  const base = await serve(t);

  assert.equal((await send(base, { method: "GET", path: "/nope" })).status, 401);
  assert.equal((await send(base, { method: "GET", path: "/mcp" })).status, 401);
  assert.equal((await send(base, { method: "DELETE", path: "/mcp" })).status, 401);
});

test("with a token, an unknown path is a 404 and a method other than POST is a 405", async (t) => {
  const base = await serve(t);
  const headers = { ...JSON_RPC, Authorization: "Bearer glpat-token" };

  const missing = await send(base, { method: "GET", path: "/nope", headers });
  assert.equal(missing.status, 404);
  assert.equal(missing.json.error.message, "Not found");

  for (const method of ["GET", "DELETE", "PUT"]) {
    const refused = await send(base, { method, headers });
    assert.equal(refused.status, 405, method);
    assert.equal(refused.headers.allow, "POST");
  }
});

test("a body that is not JSON is a 400, and one over the limit is a 413", async (t) => {
  const base = await serve(t);
  const headers = { ...JSON_RPC, Authorization: "Bearer glpat-token" };

  const malformed = await send(base, { headers, body: "{not json" });
  assert.equal(malformed.status, 400);
  assert.equal(malformed.json.error.code, -32700);

  const oversized = await send(base, { headers, body: Buffer.alloc(MAX_BODY_BYTES + 1, "a") });
  assert.equal(oversized.status, 413);
});

/* ------------------------------------------------------------------ the tools, with a token */

test("tools/list over HTTP lists exactly the surface", async (t) => {
  const base = await serve(t);

  const { status, json } = await rpc(base, "glpat-token", "tools/list", {});

  assert.equal(status, 200);
  assert.deepEqual(json.result.tools.map((tool) => tool.name).sort(), TOOL_NAMES);
});

test("the request's token is the one GitLab is called with, whatever the environment holds", async (t) => {
  withToken(t);
  const base = await serve(t);
  const { calls } = mockApi(t, reply(404, { message: "Not Found" }));

  const { status, json } = await rpc(base, "glpat-request_token", "tools/call", {
    name: "repo_read",
    arguments: { repo: "o/r" },
  });

  assert.equal(status, 200);
  assert.equal(json.result.isError, true);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://gitlab.com/api/v4/projects/o%2Fr?license=true");
  assert.equal(calls[0].headers.Authorization, "Bearer glpat-request_token");
});

test("a request with no token never reaches GitLab, even when the environment holds one", async (t) => {
  withToken(t);
  const base = await serve(t);
  const { calls } = mockApi(t); // any request to GitLab fails the test

  const { status } = await rpc(base, null, "tools/call", { name: "repo_read", arguments: { repo: "o/r" } });

  assert.equal(status, 401);
  assert.equal(calls.length, 0);
});

test("requests that overlap each reach GitLab with their own token", async (t) => {
  const base = await serve(t);
  const seen = new Map();
  t.mock.method(globalThis, "fetch", async (url, init) => {
    await new Promise((resolve) => setTimeout(resolve, 5 + Math.random() * 30));
    seen.set(new URL(url).pathname, init.headers.Authorization);
    return new Response(JSON.stringify({ message: "Not Found" }), { status: 404 });
  });

  const names = Array.from({ length: 8 }, (_, i) => `r${i}`);
  const answers = await Promise.all(
    names.map((name) => rpc(base, `glpat-token_${name}`, "tools/call", { name: "repo_read", arguments: { repo: `o/${name}` } })),
  );

  assert.ok(answers.every((answer) => answer.status === 200));
  for (const name of names) {
    assert.equal(seen.get(`/api/v4/projects/o%2F${name}`), `Bearer glpat-token_${name}`, name);
  }
});

test("GitLab rejecting the token names the header, and the token never reaches the caller or the log", async (t) => {
  const base = await serve(t);
  const token = "glpat-secret_value_1234";
  mockApi(t, reply(401, { message: `Bad credentials for ${token}` }));
  const logged = [];
  t.mock.method(process.stderr, "write", (chunk) => {
    logged.push(String(chunk));
    return true;
  });

  const { status, text, json } = await rpc(base, token, "tools/call", { name: "repo_read", arguments: { repo: "o/r" } });

  assert.equal(status, 200);
  assert.match(json.result.content[0].text, /GitLab rejected the token \(401\)\. Check that the Authorization: Bearer header holds a valid token/);
  assert.ok(!text.includes(token), "the response");
  assert.ok(!logged.join("").includes(token), "the log");
});

/* ------------------------------------------------------------------ the process */

/** A real `node src/index.js`, with its stderr collected, and the URL it says it serves. */
function startProcess(t, env) {
  const child = spawn(process.execPath, [ENTRY], { env: { ...process.env, ...env }, stdio: ["ignore", "pipe", "pipe"] });
  const exited = new Promise((resolve) => child.on("exit", (code, signal) => resolve({ code, signal })));
  let stderr = "";
  let stdout = "";
  child.stdout.on("data", (chunk) => (stdout += chunk));
  child.stderr.on("data", (chunk) => (stderr += chunk));
  t.after(() => child.kill("SIGKILL"));
  return { child, exited, output: () => ({ stdout, stderr }) };
}

test("MCP_TRANSPORT=http serves HTTP from a real process, ignores the environment's token, and stops on SIGTERM", async (t) => {
  const leftover = "leftover-token-value";
  const run = startProcess(t, { MCP_TRANSPORT: "http", PORT: "0", HOST: "127.0.0.1", MCP_AUTH_TOKEN: leftover });

  const base = await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no startup line: ${run.output().stderr}`)), 10_000);
    run.child.stderr.on("data", () => {
      const match = /serving over http on (http:\/\/127\.0\.0\.1:\d+)\/mcp/.exec(run.output().stderr);
      if (match) {
        clearTimeout(timer);
        resolve(match[1]);
      }
    });
  });

  assert.equal((await send(base, { method: "GET", path: "/healthz" })).status, 200);
  assert.equal((await rpc(base, null, "tools/list", {})).status, 401, "the environment's token is not a way in");
  assert.equal((await rpc(base, "glpat-token", "tools/list", {})).status, 200);

  run.child.kill("SIGTERM");
  assert.deepEqual(await run.exited, { code: 0, signal: null });

  const { stdout, stderr } = run.output();
  assert.equal(stdout, "", "nothing on stdout");
  assert.match(stderr, /MCP_AUTH_TOKEN is set in this process's environment and is ignored over HTTP/);
  assert.ok(!stderr.includes(leftover), "the value is never logged");
});

test("an unknown MCP_TRANSPORT, or a PORT that is not a port, exits 2 with the reason on stderr", async (t) => {
  for (const [env, reason] of [
    [{ MCP_TRANSPORT: "carrier-pigeon" }, /MCP_TRANSPORT must be stdio or http, got 'carrier-pigeon'/],
    [{ MCP_TRANSPORT: "http", PORT: "http" }, /PORT must be a whole number from 0 to 65535, got 'http'/],
    [{ MCP_TRANSPORT: "http", PORT: "70000" }, /PORT must be a whole number from 0 to 65535, got '70000'/],
  ]) {
    const run = startProcess(t, env);
    assert.equal((await run.exited).code, 2, JSON.stringify(env));
    assert.match(run.output().stderr, reason);
    assert.equal(run.output().stdout, "");
  }
});

test("httpSettings defaults to loopback and the GitLab server's port", () => {
  assert.deepEqual(httpSettings({}), { port: 3002, host: "127.0.0.1" });
  assert.deepEqual(httpSettings({ PORT: " 8080 ", HOST: " 0.0.0.0 " }), { port: 8080, host: "0.0.0.0" });
  assert.deepEqual(httpSettings({ PORT: "", HOST: "" }), { port: 3002, host: "127.0.0.1" });
});
