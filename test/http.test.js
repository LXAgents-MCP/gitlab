import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { request } from "node:http";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { DEFAULT_HOST, DEFAULT_PORT, MAX_BODY_BYTES } from "../src/config/config.js";
import { httpSettings, startHttpServer } from "../src/routes/http.js";
import { SERVER_NAME, VERSION } from "../src/config/version.js";
import { mockApi, reply, TOKEN, withToken, withoutToken } from "./helpers.js";
import { TOOL_NAMES } from "./surface.js";

/*
 * The HTTP transport over real sockets on loopback. The client here is `node:http`, not `fetch`,
 * because `fetch` is what the tests replace to stand in for GitLab; the two must not be the same.
 *
 * Two secrets are in play and the tests keep them apart: ACCESS is the server's own token, which a
 * client presents; TOKEN is the GitLab key, which the server holds and a client never sees.
 */

const ENTRY = join(dirname(fileURLToPath(import.meta.url)), "..", "src", "index.js");
const JSON_RPC = { "Content-Type": "application/json", Accept: "application/json, text/event-stream" };
const ACCESS = "f00dfacecafe0123456789abcdef".padEnd(64, "9");

/** A running server on a free loopback port, guarded by ACCESS, closed when the test ends. */
async function serve(t) {
  const running = await startHttpServer({ port: 0, host: "127.0.0.1", authToken: ACCESS });
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

/** A JSON-RPC call to `/mcp` carrying a bearer credential, or none when `bearer` is null. */
function rpc(base, bearer, method, params, id = 1) {
  return send(base, {
    headers: { ...JSON_RPC, ...(bearer === null ? {} : { Authorization: `Bearer ${bearer}` }) },
    body: JSON.stringify({ jsonrpc: "2.0", id, method, params }),
  });
}

const readRepo = (name = "r") => ({ name: "repo_read", arguments: { repo: `o/${name}` } });

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

test("a request without the access token is a 401 in the JSON-RPC envelope, before anything is read", async (t) => {
  const base = await serve(t);
  const refusal = { jsonrpc: "2.0", id: null };

  const missing = await send(base, { headers: JSON_RPC, body: "this is not json" });
  assert.equal(missing.status, 401, "the token is checked before the body is parsed");
  assert.equal(missing.headers["www-authenticate"], 'Bearer realm="mcp"');
  assert.deepEqual(missing.json, {
    ...refusal,
    error: { code: -32001, message: "Unauthorized: send the server's access token as Authorization: Bearer <token>" },
  });

  for (const authorization of ["Basic dXNlcjpwYXNz", "Bearer", "Bearer a b", "Token abc"]) {
    const wrong = await send(base, { headers: { ...JSON_RPC, Authorization: authorization }, body: "{}" });
    assert.equal(wrong.status, 401, authorization);
    assert.equal(wrong.headers["www-authenticate"], 'Bearer realm="mcp"', authorization);
  }
});

test("a wrong token is a 401 with invalid_token, whatever it resembles, and the answer never echoes either token", async (t) => {
  const base = await serve(t);
  const wrongTokens = [
    "not-the-token",
    ACCESS.slice(0, -1),
    `${ACCESS}x`,
    ACCESS.toUpperCase().replace(/[^A-Z0-9]/g, "Z"),
    "a".repeat(2000),
    "tokén",
    TOKEN,
  ];

  for (const token of wrongTokens) {
    const answer = await send(base, { headers: { ...JSON_RPC, Authorization: `Bearer ${token}` }, body: "{}" });
    assert.equal(answer.status, 401, token.slice(0, 20));
    assert.equal(answer.headers["www-authenticate"], 'Bearer realm="mcp", error="invalid_token"');
    assert.deepEqual(answer.json, {
      jsonrpc: "2.0",
      error: { code: -32001, message: "Unauthorized: the token is not valid" },
      id: null,
    });
    assert.ok(!answer.text.includes(ACCESS), "the access token");
    assert.ok(!answer.text.includes(token), "the token that was sent");
  }
});

test("a caller without the token cannot learn which routes exist", async (t) => {
  const base = await serve(t);

  assert.equal((await send(base, { method: "GET", path: "/nope" })).status, 401);
  assert.equal((await send(base, { method: "GET", path: "/mcp" })).status, 401);
  assert.equal((await send(base, { method: "DELETE", path: "/mcp" })).status, 401);
});

test("with the token, an unknown path is a 404 and a method other than POST is a 405", async (t) => {
  const base = await serve(t);
  const headers = { ...JSON_RPC, Authorization: `Bearer ${ACCESS}` };

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
  const headers = { ...JSON_RPC, Authorization: `Bearer ${ACCESS}` };

  const malformed = await send(base, { headers, body: "{not json" });
  assert.equal(malformed.status, 400);
  assert.equal(malformed.json.error.code, -32700);

  const oversized = await send(base, { headers, body: Buffer.alloc(MAX_BODY_BYTES + 1, "a") });
  assert.equal(oversized.status, 413);
});

/* ------------------------------------------------------------------ the tools, with the access token */

test("tools/list over HTTP lists exactly the surface", async (t) => {
  const base = await serve(t);

  const { status, json } = await rpc(base, ACCESS, "tools/list", {});

  assert.equal(status, 200);
  assert.deepEqual(json.result.tools.map((tool) => tool.name).sort(), TOOL_NAMES);
});

test("GitLab is called with GITLAB_API_KEY, and the client's access token never reaches it", async (t) => {
  withToken(t);
  const base = await serve(t);
  const { calls } = mockApi(t, reply(404, { message: "Not Found" }));

  const { status, json } = await rpc(base, ACCESS, "tools/call", readRepo());

  assert.equal(status, 200);
  assert.equal(json.result.isError, true);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://gitlab.com/api/v4/projects/o%2Fr?license=true");
  assert.equal(calls[0].headers.Authorization, `Bearer ${TOKEN}`, "the key, not the access token");
  assert.ok(!JSON.stringify(calls).includes(ACCESS), "the access token is in no part of the request to GitLab");
});

test("with the access token but no GitLab key, a tool is refused naming GITLAB_API_KEY, and GitLab is not called", async (t) => {
  withoutToken(t);
  const base = await serve(t);
  const { calls } = mockApi(t); // any request to GitLab fails the test

  const { status, json } = await rpc(base, ACCESS, "tools/call", readRepo());

  assert.equal(status, 200);
  assert.equal(json.result.isError, true);
  assert.match(json.result.content[0].text, /GITLAB_API_KEY is not set/);
  assert.equal(calls.length, 0);
});

test("a request without the access token never reaches GitLab, even when the key is set", async (t) => {
  withToken(t);
  const base = await serve(t);
  const { calls } = mockApi(t);

  assert.equal((await rpc(base, null, "tools/call", readRepo())).status, 401);
  assert.equal((await rpc(base, TOKEN, "tools/call", readRepo())).status, 401, "the GitLab key is not an access token");
  assert.equal(calls.length, 0);
});

test("requests that overlap all reach GitLab with the key and none with an access token", async (t) => {
  withToken(t);
  const base = await serve(t);
  const seen = new Map();
  t.mock.method(globalThis, "fetch", async (url, init) => {
    await new Promise((resolve) => setTimeout(resolve, 5 + Math.random() * 30));
    seen.set(new URL(url).pathname, init.headers.Authorization);
    return new Response(JSON.stringify({ message: "Not Found" }), { status: 404 });
  });

  const names = Array.from({ length: 8 }, (_, i) => `r${i}`);
  const answers = await Promise.all(names.map((name) => rpc(base, ACCESS, "tools/call", readRepo(name))));

  assert.ok(answers.every((answer) => answer.status === 200));
  for (const name of names) {
    assert.equal(seen.get(`/api/v4/projects/o%2F${name}`), `Bearer ${TOKEN}`, name);
  }
});

test("GitLab rejecting the key names GITLAB_API_KEY, and neither secret reaches the caller or the log", async (t) => {
  withToken(t);
  const base = await serve(t);
  mockApi(t, reply(401, { message: `Bad credentials for ${TOKEN}` }));
  const logged = [];
  t.mock.method(process.stderr, "write", (chunk) => {
    logged.push(String(chunk));
    return true;
  });

  const { status, text, json } = await rpc(base, ACCESS, "tools/call", readRepo());

  assert.equal(status, 200);
  assert.match(json.result.content[0].text, /GitLab rejected the token \(401\)\. Check that GITLAB_API_KEY holds a valid token/);
  for (const secret of [TOKEN, ACCESS]) {
    assert.ok(!text.includes(secret), "the response");
    assert.ok(!logged.join("").includes(secret), "the log");
  }
});

/* ------------------------------------------------------------------ starting */

test("startHttpServer will not listen without an access token of 32 characters, and names the variable", async () => {
  await assert.rejects(startHttpServer({ port: 0, authToken: "" }), /^Error: MCP_AUTH_TOKEN is not set$/);
  await assert.rejects(startHttpServer({ port: 0, authToken: "short" }), /MCP_AUTH_TOKEN is 5 characters, and it must be at least 32/);
});

test("httpSettings defaults to shared-instruction's: port 3000 on every interface", () => {
  assert.deepEqual(httpSettings({}), { port: 3000, host: "0.0.0.0" });
  assert.deepEqual(httpSettings({ PORT: "", HOST: "" }), { port: 3000, host: "0.0.0.0" });
  assert.deepEqual(httpSettings({ PORT: "   ", HOST: "   " }), { port: 3000, host: "0.0.0.0" });
  assert.equal(DEFAULT_PORT, 3000);
  assert.equal(DEFAULT_HOST, "0.0.0.0");
});

test("httpSettings lets PORT and HOST override, accepts 0, and refuses a PORT that is not a port", () => {
  assert.deepEqual(httpSettings({ PORT: " 8080 ", HOST: " 127.0.0.1 " }), { port: 8080, host: "127.0.0.1" });
  assert.deepEqual(httpSettings({ PORT: "0" }), { port: 0, host: "0.0.0.0" }, "0 asks the system for a free port");
  for (const bad of ["abc", "70000", "-1", "80.5", "1e9"]) {
    assert.throws(() => httpSettings({ PORT: bad }), new RegExp(`PORT must be a whole number from 0 to 65535, got '${bad}'`), bad);
  }
});

/* ------------------------------------------------------------------ the process */

/** A real `node src/index.js` with exactly the given environment on top of ours, minus both secrets. */
function startProcess(t, env) {
  const base = { ...process.env };
  delete base.GITLAB_API_KEY;
  delete base.MCP_AUTH_TOKEN;
  const child = spawn(process.execPath, [ENTRY], { env: { ...base, ...env }, stdio: ["ignore", "pipe", "pipe"] });
  const exited = new Promise((resolve) => child.on("exit", (code, signal) => resolve({ code, signal })));
  let stderr = "";
  let stdout = "";
  child.stdout.on("data", (chunk) => (stdout += chunk));
  child.stderr.on("data", (chunk) => (stderr += chunk));
  t.after(() => child.kill("SIGKILL"));
  return { child, exited, output: () => ({ stdout, stderr }) };
}

/** A port nothing is listening on right now. */
function freePort() {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const { port } = probe.address();
      probe.close(() => resolve(port));
    });
  });
}

/** The exit code of a process that should end by itself, or "running" if it is still up after `ms`: a failure that does not hang the suite. */
async function exitCodeWithin(run, ms = 10_000) {
  let timer;
  const limit = new Promise((resolve) => {
    timer = setTimeout(() => resolve("running"), ms);
  });
  const result = await Promise.race([run.exited.then((exit) => exit.code), limit]);
  clearTimeout(timer);
  return result;
}

test("MCP_TRANSPORT=http serves HTTP from a real process behind the access token and stops on SIGTERM", async (t) => {
  const key = "glpat-process_key_value_9876";
  const run = startProcess(t, { MCP_TRANSPORT: "http", PORT: "0", HOST: "127.0.0.1", MCP_AUTH_TOKEN: ACCESS, GITLAB_API_KEY: key });

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
  assert.equal((await rpc(base, null, "tools/list", {})).status, 401);
  assert.equal((await rpc(base, key, "tools/list", {})).status, 401, "the GitLab key is not a way in");
  assert.equal((await rpc(base, ACCESS, "tools/list", {})).status, 200);

  run.child.kill("SIGTERM");
  assert.deepEqual(await run.exited, { code: 0, signal: null });

  const { stdout, stderr } = run.output();
  assert.equal(stdout, "", "nothing on stdout");
  assert.ok(!stderr.includes(ACCESS) && !stderr.includes(key), "neither secret is ever logged");
  assert.doesNotMatch(stderr, /GITLAB_API_KEY is not set/, "the key was set");
});

test("with no HOST set, a real process listens on every interface and says so; with HOST set, only there", async (t) => {
  for (const [host, said] of [[undefined, /serving over http on :\d+\/mcp \(all interfaces\)/], ["127.0.0.1", /serving over http on http:\/\/127\.0\.0\.1:\d+\/mcp/]]) {
    const port = await freePort();
    const run = startProcess(t, { MCP_TRANSPORT: "http", PORT: String(port), MCP_AUTH_TOKEN: ACCESS, ...(host ? { HOST: host } : {}) });

    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`no startup line: ${run.output().stderr}`)), 10_000);
      run.child.stderr.on("data", () => {
        if (/serving over http on/.test(run.output().stderr)) {
          clearTimeout(timer);
          resolve();
        }
      });
    });

    assert.match(run.output().stderr, said, String(host));
    assert.equal((await send(`http://127.0.0.1:${port}`, { method: "GET", path: "/healthz" })).status, 200);
    run.child.kill("SIGTERM");
    await run.exited;
  }
});

test("HTTP without a GitLab key starts and says so, because a tool will refuse", async (t) => {
  const run = startProcess(t, { MCP_TRANSPORT: "http", PORT: "0", MCP_AUTH_TOKEN: ACCESS });

  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no startup line: ${run.output().stderr}`)), 10_000);
    run.child.stderr.on("data", () => {
      if (/serving over http on/.test(run.output().stderr)) {
        clearTimeout(timer);
        resolve();
      }
    });
  });

  assert.match(run.output().stderr, /GITLAB_API_KEY is not set in this process's environment: every tool will refuse until it is/);
  run.child.kill("SIGTERM");
  await run.exited;
});

test("HTTP will not start without a usable access token: it exits 1, names the variable, and listens on nothing", async (t) => {
  for (const [env, reason] of [
    [{}, /MCP_AUTH_TOKEN is not set/],
    [{ MCP_AUTH_TOKEN: "   " }, /MCP_AUTH_TOKEN is not set/],
    [{ MCP_AUTH_TOKEN: "too-short" }, /MCP_AUTH_TOKEN is 9 characters, and it must be at least 32/],
    [{ GITLAB_API_KEY: "glpat-only_the_key_1234" }, /MCP_AUTH_TOKEN is not set/],
  ]) {
    const run = startProcess(t, { MCP_TRANSPORT: "http", PORT: "0", ...env });
    assert.equal(await exitCodeWithin(run), 1, JSON.stringify(env));
    const { stdout, stderr } = run.output();
    assert.match(stderr, reason);
    assert.match(stderr, /will not start the HTTP transport/);
    assert.match(stderr, /openssl rand -hex 32/);
    assert.equal(stdout, "");
    assert.doesNotMatch(stderr, /serving over http/);
    assert.ok(!stderr.includes("too-short"), "the value is never logged");
  }
});

test("an unknown MCP_TRANSPORT, or a PORT that is not a port, exits 2 with the reason on stderr", async (t) => {
  for (const [env, reason] of [
    [{ MCP_TRANSPORT: "carrier-pigeon" }, /MCP_TRANSPORT must be stdio or http, got 'carrier-pigeon'/],
    [{ MCP_TRANSPORT: "http", PORT: "http", MCP_AUTH_TOKEN: ACCESS }, /PORT must be a whole number from 0 to 65535, got 'http'/],
    [{ MCP_TRANSPORT: "http", PORT: "70000", MCP_AUTH_TOKEN: ACCESS }, /PORT must be a whole number from 0 to 65535, got '70000'/],
  ]) {
    const run = startProcess(t, env);
    assert.equal(await exitCodeWithin(run), 2, JSON.stringify(env));
    assert.match(run.output().stderr, reason);
    assert.equal(run.output().stdout, "");
  }
});
