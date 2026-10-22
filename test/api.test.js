import assert from "node:assert/strict";
import { createServer } from "node:http";
import test from "node:test";
import {
  SEGMENT,
  apiDetail,
  callApi,
  filePath,
  gitlab,
  pageQuery,
  paging,
  parseRepo,
  redact,
  refName,
  refPath,
  requireToken,
} from "../src/services/api.js";
import { API_TIMEOUT_MS } from "../src/config/config.js";
import { Refused } from "../src/utils/errors.js";
import { VERSION } from "../src/config/version.js";
import { TOKEN, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

/** Set an environment variable for one test and put it back afterwards. */
function env(t, name, value) {
  const saved = process.env[name];
  if (value === undefined) delete process.env[name];
  else process.env[name] = value;
  t.after(() => {
    if (saved === undefined) delete process.env[name];
    else process.env[name] = saved;
  });
}

/* ------------------------------------------------------------------ the token */

test("requireToken returns the trimmed GitLab key", (t) => {
  env(t, "GITLAB_API_KEY", "  abc123  ");

  assert.equal(requireToken(), "abc123");
});

test("requireToken refuses a missing or blank token and names the variable", (t) => {
  env(t, "GITLAB_API_KEY", undefined);
  assert.throws(() => requireToken(), isRefusal(/^GITLAB_API_KEY is not set\. .*`env` block/));

  env(t, "GITLAB_API_KEY", "   ");
  assert.throws(() => requireToken(), isRefusal(/GITLAB_API_KEY is not set/));
});

test("requireToken reads the GitLab variable by default", (t) => {
  withoutToken(t);
  assert.throws(() => requireToken(), isRefusal(/^GITLAB_API_KEY is not set\./));
});

test("redact removes every occurrence of a secret and tolerates an empty one", () => {
  assert.equal(redact("a s3cret b s3cret", "s3cret"), "a *** b ***");
  assert.equal(redact("nothing here", "s3cret"), "nothing here");
  assert.equal(redact("kept", ""), "kept");
});

/* ------------------------------------------------------------------ callApi */

test("callApi sends the method, headers and a JSON body, and decodes the answer", async (t) => {
  const fetch = t.mock.method(globalThis, "fetch", async () => new Response('{"ok":true}', { status: 201 }));

  const result = await callApi("https://api.example.com/things", {
    method: "POST",
    headers: { Authorization: "Bearer x" },
    json: { a: 1 },
  });

  assert.equal(result.status, 201);
  assert.deepEqual(result.body, { ok: true });
  assert.equal(typeof result.headers, "object");
  const [url, init] = fetch.mock.calls[0].arguments;
  assert.equal(url, "https://api.example.com/things");
  assert.equal(init.method, "POST");
  assert.deepEqual(init.headers, { Authorization: "Bearer x", "Content-Type": "application/json" });
  assert.equal(init.body, '{"a":1}');
  assert.ok(init.signal instanceof AbortSignal);
});

test("callApi hands back the response headers, lowercased", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("[]", { status: 200, headers: { "X-Next-Page": "2" } }));

  const { headers } = await callApi("https://api.example.com/things");

  assert.equal(headers["x-next-page"], "2");
});

test("callApi sends no body and no content type when there is no JSON", async (t) => {
  const fetch = t.mock.method(globalThis, "fetch", async () => new Response(null, { status: 204 }));

  const result = await callApi("https://api.example.com/things");

  assert.equal(result.status, 204);
  assert.equal(result.body, null);
  const [, init] = fetch.mock.calls[0].arguments;
  assert.equal(init.method, "GET");
  assert.equal(init.body, undefined);
  assert.deepEqual(init.headers, {});
});

test("callApi hands back a body that is not JSON as short text", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("x".repeat(1000), { status: 502 }));

  const { status, body } = await callApi("https://api.example.com/things");

  assert.equal(status, 502);
  assert.equal(body, "x".repeat(300));
});

test("callApi keeps the whole body as text when asked for it raw", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("y".repeat(1000), { status: 200 }));

  const { body } = await callApi("https://api.example.com/log", { raw: true });

  assert.equal(body, "y".repeat(1000));
});

test("callApi still decodes an error answer when the body of a good one is asked for raw", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response(JSON.stringify({ message: "404 Not found" }), { status: 404 }));

  const { status, body } = await callApi("https://api.example.com/log", { raw: true });

  assert.equal(status, 404);
  assert.deepEqual(body, { message: "404 Not found" });
});

test("callApi follows a redirect to another host without sending the token there", async (t) => {
  // GitLab moves a project by redirecting a read to its new address, and a runtime forwards only
  // the headers it knows to be credentials. The token travels in `Authorization`, which it drops
  // when the host changes; this holds the client to that, over real sockets.
  const seen = [];
  const target = createServer((request, response) => {
    seen.push(request.headers.authorization ?? null);
    response.end("the body");
  });
  await new Promise((resolve) => target.listen(0, "127.0.0.1", resolve));
  const origin = createServer((request, response) => {
    response.writeHead(302, { location: `http://127.0.0.1:${target.address().port}/elsewhere` });
    response.end();
  });
  await new Promise((resolve) => origin.listen(0, "127.0.0.1", resolve));
  t.after(() => {
    for (const server of [origin, target]) {
      server.closeAllConnections();
      server.close();
    }
  });

  const { status, body } = await callApi(`http://127.0.0.1:${origin.address().port}/start`, {
    raw: true,
    headers: { Authorization: "Bearer secret" },
  });

  assert.equal(status, 200);
  assert.equal(body, "the body");
  assert.deepEqual(seen, [null]);
});

test("callApi refuses an answer larger than the client will read", async (t) => {
  t.mock.method(
    globalThis,
    "fetch",
    async () =>
      new Response(
        new ReadableStream({
          pull(controller) {
            controller.enqueue(new Uint8Array(5_000_000));
          },
        }),
        { status: 200 },
      ),
  );

  await assert.rejects(callApi("https://api.example.com/big", { raw: true }), isRefusal(/larger than 20 MB/));
});

test("callApi turns a network failure into a refusal with the secret scrubbed", async (t) => {
  t.mock.method(globalThis, "fetch", async () => {
    throw new TypeError("fetch failed", { cause: new Error("getaddrinfo ENOTFOUND for s3cret") });
  });

  await assert.rejects(
    callApi("https://api.example.com/things", { token: "s3cret" }),
    (e) =>
      e instanceof Refused &&
      e.message === "Could not reach api.example.com: getaddrinfo ENOTFOUND for ***. Check the network connection.",
  );
});

test("callApi turns a timeout into a refusal that says how long it waited", async (t) => {
  t.mock.method(globalThis, "fetch", async () => {
    throw new DOMException("The operation was aborted due to timeout", "TimeoutError");
  });

  await assert.rejects(
    callApi("https://api.example.com/things"),
    isRefusal(new RegExp(`^api\\.example\\.com did not answer within ${API_TIMEOUT_MS / 1000} seconds`)),
  );
});

/* ------------------------------------------------------------------ gitlab() */

test("gitlab sends the token as a bearer credential, builds the query, and returns the answer", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { ok: 1 }, { "x-thing": "1" }));

  const answer = await gitlab("GET", "/projects/o%2Fr/things", {
    query: { page: 2, per_page: 30, ref_name: "main", empty: "", missing: undefined, none: null },
  });

  assert.equal(answer.status, 200);
  assert.deepEqual(answer.body, { ok: 1 });
  assert.equal(answer.headers["x-thing"], "1");
  assert.equal(calls[0].url, "https://gitlab.com/api/v4/projects/o%2Fr/things?page=2&per_page=30&ref_name=main");
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[0].headers.Authorization, `Bearer ${TOKEN}`);
  assert.equal(calls[0].headers["PRIVATE-TOKEN"], undefined, "GitLab's own header is forwarded across hosts");
  assert.equal(calls[0].headers.Accept, "application/json");
  assert.equal(calls[0].headers["User-Agent"], `lxagents-mcp-gitlab/${VERSION}`);
  assert.equal(calls[0].headers["Content-Type"], undefined);
});

test("gitlab keeps an encoded slash in the path as one segment", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await gitlab("GET", `/projects/${encodeURIComponent("group/sub/project")}/repository/branches/${encodeURIComponent("feat/x")}`);

  assert.equal(calls[0].url, "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproject/repository/branches/feat%2Fx");
});

test("gitlab sends a JSON body with its content type, and a different Accept when asked", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { id: 1 }));

  await gitlab("POST", "/projects/1/things", { json: { a: 1 }, accept: "text/plain" });

  assert.deepEqual(calls[0].body, { a: 1 });
  assert.equal(calls[0].headers["Content-Type"], "application/json");
  assert.equal(calls[0].headers.Accept, "text/plain");
});

test("gitlab with no token refuses before it sends anything", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(gitlab("GET", "/user"), isRefusal(/^GITLAB_API_KEY is not set\./));
  assert.equal(calls.length, 0);
});

test("gitlab refuses a path that could change which endpoint is called", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  for (const path of ["user", "/user?x=1", "/user#frag"]) {
    await assert.rejects(gitlab("GET", path), (e) => e instanceof Error && !(e instanceof Refused), path);
  }
  assert.equal(calls.length, 0);
});

test("gitlab returns a status listed in allow instead of refusing", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "404 Tag Not Found" }));

  const answer = await gitlab("GET", "/projects/1/repository/tags/v9", { allow: [404] });

  assert.equal(answer.status, 404);
});

test("401 says the token was rejected", async (t) => {
  withToken(t);
  mockApi(t, reply(401, { message: "401 Unauthorized" }));

  await assert.rejects(
    gitlab("GET", "/user", { what: "read the user" }),
    isRefusal(/^GitLab rejected the token \(401\)\. Check that GITLAB_API_KEY holds a valid token/),
  );
});

test("403 says what permission is missing", async (t) => {
  withToken(t);
  mockApi(t, reply(403, { message: "403 Forbidden" }));

  await assert.rejects(
    gitlab("POST", "/projects/1/repository/files/a", { what: "write the file 'a' in o/r" }),
    isRefusal(/^GitLab refused to write the file 'a' in o\/r \(403\): 403 Forbidden\. .*`api` scope.*`read_api`.*role/),
  );
});

test("a 429 with retry-after says how long to wait, and one with only a reset says when", async (t) => {
  withToken(t);
  const reset = String(Math.floor(Date.now() / 1000) + 600);
  mockApi(
    t,
    reply(429, { message: "Retry later" }, { "retry-after": "30" }),
    reply(429, { message: { error: "Retry later" } }, { "ratelimit-reset": reset }),
    reply(429, "Retry later"),
  );

  await assert.rejects(gitlab("GET", "/user"), isRefusal(/^GitLab is rate limiting this token \(429\): Retry later\. Wait 30 seconds and try again\.$/));
  await assert.rejects(
    gitlab("GET", "/user"),
    isRefusal(/\(429\): error Retry later\. The limit resets at .*in about (9|10|11) minutes\.$/),
  );
  await assert.rejects(gitlab("GET", "/user"), isRefusal(/\(429\): Retry later\. Wait a minute and try again\.$/));
});

test("404 says to check the spelling and that a private project answers 404", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "404 Project Not Found" }));

  await assert.rejects(
    gitlab("GET", "/projects/o%2Fr", { what: "read the project o/r" }),
    isRefusal(/^GitLab could not find what it needs to read the project o\/r \(404\): 404 Project Not Found\. .*private project the token cannot see\.$/),
  );
});

test("405 points at a project that was renamed or moved", async (t) => {
  withToken(t);
  mockApi(t, reply(405, { message: "Non GET methods are not allowed for moved projects" }));

  await assert.rejects(
    gitlab("DELETE", "/projects/o%2Fold", { what: "delete o/old" }),
    isRefusal(/^GitLab refused to delete o\/old \(405\): Non GET methods are not allowed for moved projects\. .*current path or its numeric id\.$/),
  );
});

test("409, 400 and 422 carry GitLab's reasons, whichever shape it uses", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(409, { message: "Branch already exists" }),
    reply(400, { message: { name: ["has already been taken"], path: ["has already been taken", "is reserved"] } }),
    reply(422, { message: ["first", "second"] }),
  );

  await assert.rejects(gitlab("POST", "/x", { what: "create the branch" }), isRefusal(/could not create the branch \(409\): Branch already exists\. .*read it again/));
  await assert.rejects(
    gitlab("POST", "/x", { what: "create it" }),
    isRefusal(/^GitLab could not create it \(400\): name has already been taken; path has already been taken, is reserved\.$/),
  );
  await assert.rejects(gitlab("POST", "/x", { what: "merge it" }), isRefusal(/^GitLab could not merge it \(422\): first; second\.$/));
});

test("a 5xx says GitLab failed, and any other status is reported with its number", async (t) => {
  withToken(t);
  mockApi(t, reply(503, { message: "Unavailable" }), reply(418, { message: "Teapot" }));

  await assert.rejects(gitlab("GET", "/x", { what: "read x" }), isRefusal(/^GitLab failed to read x \(503\): Unavailable\. Try again shortly\.$/));
  await assert.rejects(gitlab("GET", "/x", { what: "read x" }), isRefusal(/^GitLab answered 418: Teapot when asked to read x\.$/));
});

test("a tool's own wording wins, and undefined falls through to the general wording", async (t) => {
  withToken(t);
  mockApi(t, reply(422, { message: "bad" }), reply(404, { message: "gone" }));
  const explain = (status, detail) => (status === 422 ? `custom ${status}${detail}` : undefined);

  await assert.rejects(gitlab("POST", "/x", { explain }), isRefusal(/^custom 422: bad$/));
  await assert.rejects(gitlab("GET", "/x", { explain, what: "find x" }), isRefusal(/^GitLab could not find what it needs to find x \(404\)/));
});

test("a failure never carries the token, even when GitLab echoes it", async (t) => {
  withToken(t);
  mockApi(t, reply(500, { message: `bad token ${TOKEN}` }));

  await assert.rejects(
    gitlab("GET", "/x"),
    (e) => e instanceof Refused && !e.message.includes(TOKEN) && e.message.includes("***"),
  );
});

test("apiDetail reads each shape GitLab answers in, and plain text", () => {
  assert.equal(apiDetail({ message: "m" }), ": m");
  assert.equal(apiDetail({ message: ["a", "b"] }), ": a; b");
  assert.equal(apiDetail({ message: [] }), "");
  assert.equal(apiDetail({ message: { name: ["taken"], base: "broken" } }), ": name taken; base broken");
  assert.equal(apiDetail({ message: {} }), "");
  assert.equal(apiDetail({ error: "invalid_token" }), ": invalid_token");
  assert.equal(apiDetail({ error: "insufficient_scope", error_description: "needs api" }), ": insufficient_scope (needs api)");
  assert.equal(apiDetail("plain"), ": plain");
  assert.equal(apiDetail("<!DOCTYPE html><html><title>Just a moment...</title>"), ": the answer was a web page and not the API's, such as a security check or an outage");
  assert.equal(apiDetail(""), "");
  assert.equal(apiDetail(null), "");
  assert.equal(apiDetail({ other: 1 }), "");
});

/* ------------------------------------------------------------------ addressing */

test("parseRepo reads a project's full path and builds the API prefix with the slashes encoded", () => {
  assert.deepEqual(parseRepo(" lxagents-mcp/gitlab "), {
    full: "lxagents-mcp/gitlab",
    path: "/projects/lxagents-mcp%2Fgitlab",
  });
  assert.deepEqual(parseRepo("group/sub.group/sub-2/my_project.v2"), {
    full: "group/sub.group/sub-2/my_project.v2",
    path: "/projects/group%2Fsub.group%2Fsub-2%2Fmy_project.v2",
  });
});

test("parseRepo reads a numeric id as it is", () => {
  assert.deepEqual(parseRepo("278964"), { full: "278964", path: "/projects/278964" });
  assert.deepEqual(parseRepo(" 7 "), { full: "7", path: "/projects/7" });
});

test("parseRepo refuses anything that is not a project path or an id", () => {
  const bad = ["", "  ", "solo", "a/", "/b", "a//b", "a/..", "a/.", "../b", "a/b c", "-a/b", ".a/b", "a/b?x=1", "a/b#c", "a b/c", "0", "007", "12abc", "-5", "1.5"];
  for (const value of bad) {
    assert.throws(() => parseRepo(value), (e) => e instanceof Refused && /`repo`/.test(e.message), JSON.stringify(value));
  }
  assert.throws(() => parseRepo(undefined), isRefusal(/`repo` is required/));
  assert.throws(() => parseRepo(5), isRefusal(/`repo` must be a string, got number/));
  assert.throws(() => parseRepo("nope", "target"), isRefusal(/`target` 'nope' is not a project\. Write its full path, such as 'my-group\/my-project'/));
  assert.throws(() => parseRepo(Array.from({ length: 22 }, () => "g").join("/")), isRefusal(/is not a project/));
  assert.throws(() => parseRepo("a/" + "b".repeat(256)), isRefusal(/is not a project/));
});

test("SEGMENT is GitLab's rule for one part of a path", () => {
  for (const value of ["a", "A1", "_x", "a.b", "a-b_c", "x".repeat(255)]) assert.match(value, SEGMENT);
  for (const value of ["", "-a", ".a", "a b", "a/b", "x".repeat(256)]) assert.doesNotMatch(value, SEGMENT);
});

test("refName accepts what git accepts", () => {
  for (const value of ["main", "feature/login", "v1.2.3", "HEAD", "0123456789abcdef0123456789abcdef01234567", "release/1.0.0", "ünï"]) {
    assert.equal(refName(value, "ref"), value);
  }
  assert.equal(refName("  main ", "ref"), "main");
});

test("refName refuses what could change the path or that git itself refuses", () => {
  const bad = ["", "a..b", "a b", "-x", "a/", "/a", "a//b", ".hidden", "a/.hidden", "x.lock", "a/x.lock", "a~1", "a^", "a:b", "a?b", "a*b", "a[b", "a\\b", "a@{b", "end.", "a\nb", "a".repeat(256)];
  for (const value of bad) {
    assert.throws(() => refName(value, "ref"), (e) => e instanceof Refused, JSON.stringify(value));
  }
  assert.throws(() => refName("a b", "branch"), isRefusal(/^`branch` 'a b' is not a valid branch, tag or commit name\.$/));
});

test("refPath is one path segment, the slashes encoded, as GitLab reads a branch name", () => {
  assert.equal(refPath("feature/login"), "feature%2Flogin");
  assert.equal(refPath("a#b/c d"), "a%23b%2Fc%20d");
  assert.equal(refPath("100%"), "100%25");
  assert.equal(refPath("main"), "main");
});

test("filePath cleans a path and encodes it as the one segment GitLab's files API takes", () => {
  assert.deepEqual(filePath("/src/a b.js/", "path"), { path: "src/a b.js", encoded: "src%2Fa%20b.js" });
  assert.deepEqual(filePath("README.md", "path"), { path: "README.md", encoded: "README.md" });
  assert.deepEqual(filePath("dir/ünï#.txt", "path"), { path: "dir/ünï#.txt", encoded: "dir%2F%C3%BCn%C3%AF%23.txt" });
});

test("filePath treats the root as valid only when allowed, and refuses traversal", () => {
  assert.deepEqual(filePath(undefined, "path", { allowEmpty: true }), { path: "", encoded: "" });
  assert.deepEqual(filePath("/", "path", { allowEmpty: true }), { path: "", encoded: "" });
  assert.throws(() => filePath("", "path"), isRefusal(/`path` is required/));
  for (const value of ["../x", "a/../b", "a/./b", "a//b", "..", "a\u0000b"]) {
    assert.throws(() => filePath(value, "path"), isRefusal(/is not a path inside the repository/), JSON.stringify(value));
  }
  assert.throws(() => filePath(5, "path"), isRefusal(/`path` must be a string/));
});

/* ------------------------------------------------------------------ paging */

test("pageQuery defaults to the first page of thirty and bounds what it is given", () => {
  assert.deepEqual(pageQuery({}), { page: 1, per_page: 30 });
  assert.deepEqual(pageQuery({ page: 3, per_page: 100 }), { page: 3, per_page: 100 });
  assert.throws(() => pageQuery({ page: 0 }), isRefusal(/`page` must be between 1 and/));
  assert.throws(() => pageQuery({ per_page: 101 }), isRefusal(/`per_page` must be between 1 and 100, got 101\./));
  assert.throws(() => pageQuery({ page: "2" }), isRefusal(/`page` must be a whole number/));
});

test("paging says whether GitLab has another page", () => {
  const query = { page: 2, per_page: 30 };

  assert.deepEqual(paging(query, { "x-next-page": "3", "x-page": "2" }), { page: 2, per_page: 30, has_more: true });
  assert.equal(paging(query, { "x-next-page": "" }).has_more, false);
  assert.equal(paging(query, { link: '<https://gitlab.com/x?page=3>; rel="next", <y>; rel="first"' }).has_more, true);
  assert.equal(paging(query, { link: '<https://gitlab.com/x?page=1>; rel="prev"' }).has_more, false);
  assert.equal(paging(query, {}).has_more, false);
});
