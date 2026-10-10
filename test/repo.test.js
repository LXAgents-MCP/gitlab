import assert from "node:assert/strict";
import test from "node:test";
import { TOKEN_ENV } from "../src/config.js";
import { Refused } from "../src/errors.js";
import { TOOL as create, handle as repoCreate } from "../src/tools/repo-create.js";
import { TOOL as del, handle as repoDelete } from "../src/tools/repo-delete.js";
import { TOOL as fork, handle as repoFork } from "../src/tools/repo-fork.js";
import { TOOL as read, handle as repoRead } from "../src/tools/repo-read.js";
import { TOOL as search, handle as repoSearch } from "../src/tools/repo-search.js";
import { TOOL as update, handle as repoUpdate } from "../src/tools/repo-update.js";
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

/* ------------------------------------------------------------------ the other project tools */

const API = "https://gitlab.com/api/v4";

/** A project as GitLab returns it to a token. */
const full = (overrides = {}) => ({
  id: 7,
  name: "r",
  path: "r",
  path_with_namespace: "g/r",
  description: "A thing",
  web_url: "https://gitlab.com/g/r",
  http_url_to_repo: "https://gitlab.com/g/r.git",
  visibility: "public",
  default_branch: "main",
  archived: false,
  star_count: 5,
  forks_count: 2,
  open_issues_count: 3,
  license: { key: "mit", name: "MIT License" },
  topics: ["mcp"],
  last_activity_at: "2026-10-01T00:00:00.000Z",
  ...overrides,
});

const FACTS = {
  id: 7,
  full_name: "g/r",
  description: "A thing",
  html_url: "https://gitlab.com/g/r",
  clone_url: "https://gitlab.com/g/r.git",
  visibility: "public",
  default_branch: "main",
  fork: false,
  archived: false,
  stars: 5,
  forks: 2,
  open_issues: 3,
  license: "MIT License",
  topics: ["mcp"],
  last_activity_at: "2026-10-01T00:00:00.000Z",
};

test("repo_read reads one project, asks for its license, and reports its facts", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, full()));

  const { payload, ok } = await repoRead({ repo: "g/r" });

  assert.equal(ok, true);
  assert.deepEqual(payload, { summary: "g/r is a public project, default branch main.", ...FACTS });
  expectOutput(read, payload);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr?license=true`);
});

test("repo_read takes a numeric id, and a nested group's path as one encoded segment", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, full()), reply(200, full({ path_with_namespace: "a/b/c" })));

  await repoRead({ repo: "278964" });
  await repoRead({ repo: "a/b/c" });

  assert.equal(calls[0].url, `${API}/projects/278964?license=true`);
  assert.equal(calls[1].url, `${API}/projects/a%2Fb%2Fc?license=true`);
});

test("repo_read says when a project is an archived fork scheduled for deletion, and leaves out what GitLab did not give", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(
      200,
      full({
        archived: true,
        description: null,
        license: null,
        topics: undefined,
        tag_list: ["old"],
        last_activity_at: undefined,
        default_branch: undefined,
        forked_from_project: { path_with_namespace: "up/stream" },
        marked_for_deletion_on: "2026-11-09",
      }),
    ),
  );

  const { payload } = await repoRead({ repo: "g/r" });

  assert.equal(payload.summary, "g/r is a public archived fork, scheduled for deletion on 2026-11-09.");
  assert.equal(payload.parent, "up/stream");
  assert.equal(payload.scheduled_for_deletion, "2026-11-09");
  assert.deepEqual(payload.topics, ["old"]);
  for (const absent of ["description", "license", "last_activity_at", "default_branch"]) {
    assert.equal(absent in payload, false, absent);
  }
  expectOutput(read, payload);
});

test("repo_read reads what an anonymous answer leaves out as nothing", async (t) => {
  withToken(t);
  mockApi(t, reply(200, full({ archived: undefined, open_issues_count: undefined, star_count: undefined, forks_count: undefined })));

  const { payload } = await repoRead({ repo: "g/r" });

  assert.equal(payload.archived, false);
  assert.deepEqual([payload.stars, payload.forks, payload.open_issues], [0, 0, 0]);
  expectOutput(read, payload);
});

test("repo_read refuses a bad repo before any request, a 404 with the cause, and an unexpected body", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "404 Project Not Found" }), reply(200, { nope: true }));

  await assert.rejects(repoRead({ repo: "nope" }), isRefusal(/`repo` 'nope' is not a project/));
  await assert.rejects(repoRead({}), isRefusal(/`repo` is required/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    repoRead({ repo: "g/r" }),
    isRefusal(/^GitLab could not find what it needs to read the project g\/r \(404\): 404 Project Not Found\. .*private project/),
  );
  await assert.rejects(repoRead({ repo: "g/r" }), isRefusal(/the body was not a project this server recognizes/));
});

test("repo_update sends only what changes, and says what it changed", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, full({ description: "New", visibility: "private" })));

  const { payload } = await repoUpdate({ repo: "g/r", description: "New", visibility: "private", has_issues: false });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "PUT");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr`);
  assert.deepEqual(calls[0].body, { description: "New", visibility: "private", issues_access_level: "disabled" });
  assert.equal(payload.summary, "Updated g/r: description, visibility, has_issues. g/r is a private project, default branch main.");
  expectOutput(update, payload);
});

test("repo_update can clear a description, rename, move the path and change the default branch", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, full({ path_with_namespace: "g/renamed", description: null, default_branch: "trunk" })));

  const { payload } = await repoUpdate({
    repo: "g/r",
    description: "",
    name: " Renamed ",
    path: "renamed",
    default_branch: "trunk",
    has_issues: true,
  });

  assert.deepEqual(calls[0].body, {
    name: "Renamed",
    path: "renamed",
    description: "",
    default_branch: "trunk",
    issues_access_level: "enabled",
  });
  assert.equal(payload.full_name, "g/renamed");
  assert.equal("description" in payload, false);
});

test("repo_update archives with its own endpoint, after the settings, and addresses it by id", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, full({ path_with_namespace: "g/renamed", description: "New" })), reply(200, full({ path_with_namespace: "g/renamed", archived: true })));

  const { payload } = await repoUpdate({ repo: "g/r", path: "renamed", description: "New", archived: true });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].method, "PUT");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr`);
  assert.equal(calls[1].method, "POST");
  assert.equal(calls[1].url, `${API}/projects/7/archive`);
  assert.equal(calls[1].body, undefined);
  assert.equal(payload.archived, true);
  assert.equal(payload.full_name, "g/renamed");
  expectOutput(update, payload);
});

test("repo_update unarchives before it changes anything else, and archiving alone is one call", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, full()), reply(200, full({ description: "New" })), reply(200, full({ archived: true })));

  await repoUpdate({ repo: "g/r", archived: false, description: "New" });
  assert.deepEqual(
    calls.map((call) => `${call.method} ${call.url.replace(API, "")}`),
    ["POST /projects/g%2Fr/unarchive", "PUT /projects/7"],
  );

  await repoUpdate({ repo: "g/r", archived: true });
  assert.equal(calls.length, 3);
  assert.equal(`${calls[2].method} ${calls[2].url.replace(API, "")}`, "POST /projects/g%2Fr/archive");
});

test("repo_update says what had already happened when a later step is refused", async (t) => {
  withToken(t);
  mockApi(t, reply(200, full()), reply(400, { message: { default_branch: ["does not exist"] } }), reply(200, full()), reply(403, { message: "403 Forbidden" }));

  await assert.rejects(
    repoUpdate({ repo: "g/r", archived: false, default_branch: "nope" }),
    isRefusal(/default_branch does not exist\. The project had already been unarchived\.$/),
  );
  await assert.rejects(
    repoUpdate({ repo: "g/r", description: "x", archived: true }),
    isRefusal(/^GitLab refused to archive the project g\/r \(403\).*The other settings had already been changed\.$/),
  );
});

test("repo_update refuses an empty change and bad values before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, full()));

  await assert.rejects(repoUpdate({ repo: "g/r" }), isRefusal(/^Nothing to change: pass at least one of name, path,/));
  await assert.rejects(repoUpdate({ repo: "g/r", path: "has space" }), isRefusal(/not a valid GitLab project path/));
  await assert.rejects(repoUpdate({ repo: "g/r", path: "x.git" }), isRefusal(/not a valid GitLab project path/));
  await assert.rejects(repoUpdate({ repo: "g/r", visibility: "secret" }), isRefusal(/`visibility` must be one of private, internal, public, got 'secret'/));
  await assert.rejects(repoUpdate({ repo: "g/r", default_branch: "a..b" }), isRefusal(/`default_branch` 'a..b' is not a valid/));
  await assert.rejects(repoUpdate({ repo: "g/r", archived: 1 }), isRefusal(/`archived` must be true or false/));
  await assert.rejects(repoUpdate({ repo: "g/r", has_issues: "yes" }), isRefusal(/`has_issues` must be true or false/));
  await assert.rejects(repoUpdate({ repo: "g/r", description: 5 }), isRefusal(/`description` must be a string/));
  assert.equal(calls.length, 0);
});

test("repo_update carries GitLab's reason when it cannot change a setting", async (t) => {
  withToken(t);
  mockApi(t, reply(400, { message: { visibility_level: ["private is not allowed since the fork source project has public visibility."] } }));

  await assert.rejects(
    repoUpdate({ repo: "g/r", visibility: "private" }),
    isRefusal(/^GitLab could not update the project g\/r \(400\): visibility_level private is not allowed since the fork source project has public visibility\.\.$/),
  );
});

test("repo_delete deletes only when confirm repeats the full path, ignoring case", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(202, { message: "202 Accepted" }));

  const { payload } = await repoDelete({ repo: "g/r", confirm: " G/R " });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr`);
  assert.equal(payload.accepted, true);
  assert.equal(payload.full_name, "g/r");
  assert.match(payload.summary, /^GitLab accepted the deletion of g\/r\. It may mark the project for deletion and remove it after a retention period/);
  expectOutput(del, payload);
});

test("repo_delete resolves a numeric id to its path before it compares", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, full()), reply(202, { message: "202 Accepted" }), reply(200, full()));

  const { payload } = await repoDelete({ repo: "7", confirm: "g/r" });
  assert.deepEqual(calls.map((call) => `${call.method} ${call.url.replace(API, "")}`), ["GET /projects/7", "DELETE /projects/7"]);
  assert.equal(payload.full_name, "g/r");

  await assert.rejects(repoDelete({ repo: "7", confirm: "7" }), isRefusal(/must repeat the project's full path, 'g\/r'/));
  assert.equal(calls.length, 3, "only the read was sent for the refused call");
});

test("repo_delete refuses without a matching confirm, and sends nothing", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(202, {}));

  await assert.rejects(
    repoDelete({ repo: "g/r", confirm: "g/other" }),
    isRefusal(/^`confirm` must repeat the project's full path, 'g\/r', to delete it\. Nothing was deleted\.$/),
  );
  await assert.rejects(repoDelete({ repo: "g/r", confirm: "r" }), isRefusal(/Nothing was deleted/));
  await assert.rejects(repoDelete({ repo: "g/r" }), isRefusal(/`confirm` is required/));
  await assert.rejects(repoDelete({ repo: "g/r", confirm: true }), isRefusal(/`confirm` must be a string/));
  await assert.rejects(repoDelete({ confirm: "g/r" }), isRefusal(/`repo` is required/));
  assert.equal(calls.length, 0);
});

test("repo_delete says what deleting needs, and that a missing project is a 404", async (t) => {
  withToken(t);
  mockApi(t, reply(403, { message: "403 Forbidden" }), reply(404, { message: "404 Project Not Found" }));

  await assert.rejects(
    repoDelete({ repo: "g/r", confirm: "g/r" }),
    isRefusal(/^GitLab refused to delete g\/r \(403\): 403 Forbidden\. Deleting needs the Owner role on the project/),
  );
  await assert.rejects(repoDelete({ repo: "g/r", confirm: "g/r" }), isRefusal(/could not find what it needs to delete the project g\/r \(404\)/));
});

test("repo_fork forks into the token owner's namespace by default, and says GitLab finishes it later", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(201, full({ path_with_namespace: "me/r", forked_from_project: { path_with_namespace: "g/r" }, import_status: "scheduled" })),
  );

  const { payload } = await repoFork({ repo: "g/r" });

  assert.equal(calls[0].method, "POST");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/fork`);
  assert.deepEqual(calls[0].body, {});
  assert.match(payload.summary, /^Forking g\/r into me\/r has started\. GitLab finishes the copy in the background/);
  assert.equal(payload.parent, "g/r");
  assert.equal(payload.fork, true);
  assert.equal(payload.import_status, "scheduled");
  expectOutput(fork, payload);
});

test("repo_fork passes the group, the new name and the new path", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, full({ path_with_namespace: "acme/tools/copy", forked_from_project: {} })));

  const { payload } = await repoFork({ repo: "g/r", group: " acme/tools ", name: "Copy", path: "copy" });

  assert.deepEqual(calls[0].body, { namespace_path: "acme/tools", name: "Copy", path: "copy" });
  assert.equal("import_status" in payload, false);
});

test("repo_fork refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, full()));

  await assert.rejects(repoFork({ repo: "nope" }), isRefusal(/is not a project/));
  await assert.rejects(repoFork({ repo: "g/r", group: "a//b" }), isRefusal(/`group` 'a\/\/b' is not a valid GitLab namespace path/));
  await assert.rejects(repoFork({ repo: "g/r", path: "a b" }), isRefusal(/`path` 'a b' is not a valid GitLab project path/));
  await assert.rejects(repoFork({ repo: "g/r", name: 5 }), isRefusal(/`name` must be a string/));
  assert.equal(calls.length, 0);
});

test("repo_search sends the query and the paging, and reports each match", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(
      200,
      [
        { path_with_namespace: "a/mcp", description: "First", star_count: 9, last_activity_at: "2026-10-01T00:00:00Z", web_url: "https://gitlab.com/a/mcp" },
        { path_with_namespace: "b/mcp", description: null, star_count: 1, web_url: "https://gitlab.com/b/mcp" },
      ],
      { "x-total": "37", "x-next-page": "2" },
    ),
  );

  const { payload } = await repoSearch({ query: " mcp server ", sort: "star_count", order: "asc", per_page: 2 });

  const url = new URL(calls[0].url);
  assert.equal(url.origin + url.pathname, `${API}/projects`);
  assert.deepEqual(Object.fromEntries(url.searchParams), { search: "mcp server", order_by: "star_count", sort: "asc", page: "1", per_page: "2" });
  assert.deepEqual(payload, {
    summary: "37 projects match 'mcp server'; showing 2.",
    items: [
      { full_name: "a/mcp", description: "First", stars: 9, last_activity_at: "2026-10-01T00:00:00Z", url: "https://gitlab.com/a/mcp" },
      { full_name: "b/mcp", stars: 1, url: "https://gitlab.com/b/mcp" },
    ],
    total_count: 37,
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(search, payload);
});

test("repo_search leaves the direction out unless there is a sort, falls back to the page when GitLab sends no total, and reports no match plainly", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [{ path_with_namespace: "a/x", star_count: 0, web_url: "https://gitlab.com/a/x" }]), reply(200, []));

  const some = await repoSearch({ query: "x", order: "asc" });
  const searchParams = new URL(calls[0].url).searchParams;
  assert.equal(searchParams.has("order_by"), false);
  assert.equal(searchParams.has("sort"), false);
  assert.equal(some.payload.total_count, 1);
  assert.equal(some.payload.summary, "1 project matches 'x'; showing 1.");
  assert.equal(some.payload.has_more, false);

  const none = await repoSearch({ query: "zzzz" });
  assert.equal(none.payload.summary, "No projects match 'zzzz'.");
  assert.equal(none.payload.total_count, 0);
  expectOutput(search, none.payload);
});

test("repo_search refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, []));

  await assert.rejects(repoSearch({}), isRefusal(/`query` is required/));
  await assert.rejects(repoSearch({ query: "  " }), isRefusal(/`query` is required/));
  await assert.rejects(repoSearch({ query: "x", sort: "best" }), isRefusal(/`sort` must be one of name, path, created_at,.*got 'best'/));
  await assert.rejects(repoSearch({ query: "x", order: "up" }), isRefusal(/`order` must be one of asc, desc/));
  await assert.rejects(repoSearch({ query: "x", per_page: 101 }), isRefusal(/`per_page` must be between 1 and 100/));
  assert.equal(calls.length, 0);
});

test("repo_search passes on GitLab's reason for a query it cannot run, and its rate limit", async (t) => {
  withToken(t);
  mockApi(t, reply(400, { message: "search is too short" }), reply(429, { message: "Retry later" }, { "retry-after": "20" }));

  await assert.rejects(repoSearch({ query: "x" }), isRefusal(/^GitLab could not search projects for 'x' \(400\): search is too short\.$/));
  await assert.rejects(repoSearch({ query: "x" }), isRefusal(/rate limiting this token \(429\): Retry later\. Wait 20 seconds/));
});
