import assert from "node:assert/strict";
import test from "node:test";
import { TOKEN_ENV } from "../src/config.js";
import { Refused } from "../src/errors.js";
import { TOOL as create, handle as repoCreate } from "../src/tools/repo-create.js";
import { VERSION } from "../src/version.js";
import { TOKEN, expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

/* ------------------------------------------------------------------ repo_create
 * Moved from the `cli` server, where it was `create_gitlab_repo`. The behaviour is unchanged,
 * so these are that tool's tests with the new name and the shared helpers.
 */

const project = (overrides = {}) => ({
  id: 7,
  name: "demo",
  path: "demo",
  path_with_namespace: "me/demo",
  web_url: "https://gitlab.com/me/demo",
  http_url_to_repo: "https://gitlab.com/me/demo.git",
  visibility: "private",
  default_branch: "main",
  ...overrides,
});

test("repo_create: the token variable is the one the owner named", () => {
  assert.equal(TOKEN_ENV, "LXAGENTS_MCP_GITLAB_API_KEY");
  assert.equal(create.name, "repo_create");
  assert.ok(create.description.includes(TOKEN_ENV));
  assert.deepEqual(Object.keys(create.inputSchema), ["name", "group", "description", "visibility", "initialize_with_readme"]);
});

test("repo_create: a project is created in the token owner's namespace, private and initialized by default", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, project()));

  const { payload, ok } = await repoCreate({ name: "demo" });

  assert.equal(ok, true);
  assert.deepEqual(payload, {
    summary: "Created private repository me/demo on GitLab.",
    name: "demo",
    full_name: "me/demo",
    html_url: "https://gitlab.com/me/demo",
    clone_url: "https://gitlab.com/me/demo.git",
    visibility: "private",
    default_branch: "main",
  });
  expectOutput(create, payload);

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://gitlab.com/api/v4/projects");
  assert.equal(calls[0].method, "POST");
  assert.deepEqual(calls[0].body, { name: "demo", path: "demo", visibility: "private", initialize_with_readme: true });
  assert.equal(calls[0].headers.Authorization, `Bearer ${TOKEN}`);
  assert.equal(calls[0].headers["User-Agent"], `lxagents-mcp-gitlab/${VERSION}`);
  assert.equal(calls[0].headers["Content-Type"], "application/json");
});

test("repo_create: a group is looked up by its encoded path, and its id becomes the namespace", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, { id: 42, full_path: "acme/tools" }),
    reply(201, project({ path_with_namespace: "acme/tools/demo", visibility: "internal" })),
  );

  const { payload } = await repoCreate({
    name: "demo",
    group: "acme/tools",
    description: "  A demo  ",
    visibility: "internal",
    initialize_with_readme: false,
  });

  assert.equal(payload.summary, "Created internal repository acme/tools/demo on GitLab.");
  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, "https://gitlab.com/api/v4/groups/acme%2Ftools");
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[0].headers.Authorization, `Bearer ${TOKEN}`);
  assert.deepEqual(calls[1].body, {
    name: "demo",
    path: "demo",
    visibility: "internal",
    initialize_with_readme: false,
    namespace_id: 42,
    description: "A demo",
  });
});

test("repo_create: a blank group means the personal namespace and no lookup", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, project()));

  await repoCreate({ name: "demo", group: "   " });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://gitlab.com/api/v4/projects");
});

test("repo_create: a missing default branch is left out, and visibility falls back to what was asked", async (t) => {
  withToken(t);
  mockApi(t, reply(201, project({ visibility: undefined, default_branch: null })));

  const { payload } = await repoCreate({ name: "demo", visibility: "public" });

  assert.equal(payload.visibility, "public");
  assert.equal("default_branch" in payload, false);
  expectOutput(create, payload);
});

test("repo_create: a missing token is a refusal that names the variable, and nothing is sent", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(201, project()));

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/^LXAGENTS_MCP_GITLAB_API_KEY is not set\./));
  assert.equal(calls.length, 0);
});

test("repo_create: bad arguments are refused before the token is read or anything is sent", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(201, project()));

  await assert.rejects(repoCreate({}), isRefusal(/`name` is required/));
  await assert.rejects(repoCreate({ name: "   " }), isRefusal(/`name` is required/));
  await assert.rejects(repoCreate({ name: 5 }), isRefusal(/`name` must be a string/));
  await assert.rejects(repoCreate({ name: "has space" }), isRefusal(/not a valid GitLab repository name/));
  await assert.rejects(repoCreate({ name: "a/b" }), isRefusal(/not a valid GitLab repository name/));
  await assert.rejects(repoCreate({ name: "-lead" }), isRefusal(/not a valid GitLab repository name/));
  await assert.rejects(repoCreate({ name: ".lead" }), isRefusal(/not a valid GitLab repository name/));
  await assert.rejects(repoCreate({ name: "repo.git" }), isRefusal(/not a valid GitLab repository name/));
  await assert.rejects(repoCreate({ name: "repo.ATOM" }), isRefusal(/not a valid GitLab repository name/));
  await assert.rejects(repoCreate({ name: "ok", group: "a//b" }), isRefusal(/not a valid GitLab group path/));
  await assert.rejects(repoCreate({ name: "ok", group: "/a" }), isRefusal(/not a valid GitLab group path/));
  await assert.rejects(repoCreate({ name: "ok", group: Array(21).fill("g").join("/") }), isRefusal(/not a valid GitLab group path/));
  await assert.rejects(
    repoCreate({ name: "ok", visibility: "secret" }),
    isRefusal(/`visibility` must be one of private, internal, public, got 'secret'/),
  );
  await assert.rejects(repoCreate({ name: "ok", initialize_with_readme: "yes" }), isRefusal(/`initialize_with_readme` must be true or false/));
  await assert.rejects(repoCreate({ name: "ok", description: 3 }), isRefusal(/`description` must be a string/));

  assert.equal(calls.length, 0);
});

test("repo_create: 401 says the token was rejected, and stops before any project is created", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(401, { message: "401 Unauthorized" }));

  await assert.rejects(
    repoCreate({ name: "demo", group: "acme" }),
    isRefusal(/^GitLab rejected the token \(401\)\. Check that LXAGENTS_MCP_GITLAB_API_KEY holds a valid token/),
  );
  assert.equal(calls.length, 1);
});

test("repo_create: 404 on the group says the group may not exist, and no project is created", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "404 Group Not Found" }));

  await assert.rejects(
    repoCreate({ name: "demo", group: "ghost" }),
    isRefusal(/could not find the group 'ghost' \(404\).*omit `group`/),
  );
  assert.equal(calls.length, 1);
});

test("repo_create: a group answer without an id is refused", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { unexpected: true }));

  await assert.rejects(repoCreate({ name: "demo", group: "acme" }), isRefusal(/answered 200 for the group 'acme', but without an id/));
});

test("repo_create: 403 says what the token needs, for the namespace or the group", async (t) => {
  withToken(t);
  mockApi(t, reply(403, { message: "403 Forbidden" }), reply(403, { message: "403 Forbidden" }));

  await assert.rejects(
    repoCreate({ name: "demo" }),
    isRefusal(/403\): 403 Forbidden\. The token needs the `api` scope.* in the token owner's namespace\.$/),
  );
  await assert.rejects(repoCreate({ name: "demo", group: "acme" }), isRefusal(/GitLab refused the request \(403\)/));
});

test("repo_create: 403 on the create step names the group", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { id: 1 }), reply(403, { message: "403 Forbidden" }));

  await assert.rejects(repoCreate({ name: "demo", group: "acme" }), isRefusal(/create projects in 'acme'/));
});

test("repo_create: 400 carries GitLab's field errors, such as a name that is taken", async (t) => {
  withToken(t);
  mockApi(t, reply(400, { message: { name: ["has already been taken"], path: ["has already been taken"] } }));

  await assert.rejects(
    repoCreate({ name: "demo" }),
    isRefusal(
      /could not create 'demo' \(400\): name has already been taken; path has already been taken\..*already exists in the token owner's namespace/,
    ),
  );
});

test("repo_create: an OAuth-style error is reported with its description", async (t) => {
  withToken(t);
  mockApi(t, reply(403, { error: "insufficient_scope", error_description: "needs api" }));

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/\(403\): insufficient_scope \(needs api\)\./));
});

test("repo_create: any other status is reported with its status and message", async (t) => {
  withToken(t);
  mockApi(t, reply(500, { message: "500 Internal Server Error" }));

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/^GitLab answered 500: 500 Internal Server Error\.$/));
});

test("repo_create: a failure never carries the token, even when GitLab echoes it", async (t) => {
  withToken(t);
  mockApi(t, reply(500, { message: `bad token ${TOKEN}` }));

  await assert.rejects(
    repoCreate({ name: "demo" }),
    (e) => e instanceof Refused && !e.message.includes(TOKEN) && e.message.includes("***"),
  );
});

test("repo_create: a 201 whose body is not a project is refused", async (t) => {
  withToken(t);
  mockApi(t, reply(201, { unexpected: true }));

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/answered 201, but the body was not a project/));
});

test("repo_create: a network failure is a refusal", async (t) => {
  withToken(t);
  t.mock.method(globalThis, "fetch", async () => {
    throw new TypeError("fetch failed", { cause: new Error("ECONNREFUSED") });
  });

  await assert.rejects(repoCreate({ name: "demo" }), isRefusal(/^Could not reach gitlab\.com: ECONNREFUSED\./));
});
