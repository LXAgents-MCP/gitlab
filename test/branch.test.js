import assert from "node:assert/strict";
import test from "node:test";
import { TOOL as create, handle as branchCreate } from "../src/tools/branch-create.js";
import { TOOL as del, handle as branchDelete } from "../src/tools/branch-delete.js";
import { TOOL as list, handle as branchList } from "../src/tools/branch-list.js";
import { TOOL as merge, handle as branchMerge } from "../src/tools/branch-merge.js";
import { TOOL as read, handle as branchRead } from "../src/tools/branch-read.js";
import { mergeability } from "../src/merge-status.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const API = "https://gitlab.com/api/v4";
const SHA = "0123456789abcdef0123456789abcdef01234567";
const OTHER = "fedcba9876543210fedcba9876543210fedcba98";

/** A branch as GitLab returns it. */
const branch = (overrides = {}) => ({
  name: "main",
  commit: {
    id: SHA,
    short_id: SHA.slice(0, 8),
    title: "Add login",
    message: "Add login\n\nWith a form.",
    committed_date: "2026-10-01T12:00:00.000+00:00",
  },
  merged: false,
  protected: true,
  default: true,
  developers_can_push: false,
  web_url: "https://gitlab.com/g/r/-/tree/main",
  ...overrides,
});

/* ------------------------------------------------------------------ branch_create */

test("branch_create finds the default branch, then creates from it", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { default_branch: "main" }), reply(201, branch({ name: "feat/x", protected: false, default: false })));

  const { payload } = await branchCreate({ repo: "g/r", branch: "feat/x" });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, `${API}/projects/g%2Fr`);
  assert.equal(calls[1].method, "POST");
  const url = new URL(calls[1].url);
  assert.equal(url.pathname, "/api/v4/projects/g%2Fr/repository/branches");
  assert.deepEqual(Object.fromEntries(url.searchParams), { branch: "feat/x", ref: "main" });
  assert.deepEqual(payload, {
    summary: "Created branch feat/x at 0123456 in g/r, from main.",
    branch: "feat/x",
    sha: SHA,
    from: "main",
  });
  expectOutput(create, payload);
});

test("branch_create starts from the branch, tag or sha it is given, with no lookup", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, branch({ name: "hotfix" })), reply(201, branch({ name: "hotfix2" })));

  await branchCreate({ repo: "g/r", branch: "hotfix", from: "release/1.0" });
  await branchCreate({ repo: "g/r", branch: "hotfix2", from: SHA });

  assert.equal(calls.length, 2);
  assert.equal(new URL(calls[0].url).searchParams.get("ref"), "release/1.0");
  assert.equal(new URL(calls[1].url).searchParams.get("ref"), SHA);
});

test("branch_create says when the name is taken, when the source is missing, and when there is no default branch", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, { default_branch: "main" }),
    reply(400, { message: "Branch already exists" }),
    reply(400, { message: "Invalid reference name: nope" }),
    reply(200, { default_branch: null }),
  );

  await assert.rejects(
    branchCreate({ repo: "g/r", branch: "feat/x" }),
    isRefusal(/could not create the branch 'feat\/x' \(400\): Branch already exists\. A branch with that name already exists in g\/r/),
  );
  await assert.rejects(
    branchCreate({ repo: "g/r", branch: "feat/x", from: "nope" }),
    isRefusal(/\(400\): Invalid reference name: nope\. 'nope' is not a branch, tag or commit of g\/r\./),
  );
  await assert.rejects(branchCreate({ repo: "g/r", branch: "feat/x" }), isRefusal(/g\/r has no default branch.*file_create/));
});

test("branch_create refuses a body that is not a branch, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { nope: true }));

  await assert.rejects(branchCreate({ repo: "g/r" }), isRefusal(/`branch` is required/));
  await assert.rejects(branchCreate({ repo: "g/r", branch: "a b" }), isRefusal(/`branch` 'a b' is not a valid/));
  await assert.rejects(branchCreate({ repo: "g/r", branch: "ok", from: "a..b" }), isRefusal(/`from` 'a..b' is not a valid/));
  await assert.rejects(branchCreate({ repo: "nope", branch: "ok" }), isRefusal(/is not a project/));
  assert.equal(calls.length, 0);

  await assert.rejects(branchCreate({ repo: "g/r", branch: "ok", from: "main" }), isRefusal(/not a branch this server recognizes/));
});

/* ------------------------------------------------------------------ branch_read */

test("branch_read reports the tip, its title and date, and whether the branch is protected or the default", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, branch()));

  const { payload } = await branchRead({ repo: "g/r", branch: "main" });

  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/branches/main`);
  assert.deepEqual(payload, {
    summary: "Branch main of g/r is at 0123456, the default and is protected.",
    name: "main",
    sha: SHA,
    protected: true,
    default: true,
    message: "Add login",
    date: "2026-10-01T12:00:00.000+00:00",
  });
  expectOutput(read, payload);
});

test("branch_read writes a slash in the name as one encoded segment, which is the only form GitLab finds", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, branch({ name: "feat/a#b", protected: false, default: false })));

  const { payload } = await branchRead({ repo: "g/r", branch: "feat/a#b" });

  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/branches/feat%2Fa%23b`);
  assert.equal(payload.summary, "Branch feat/a#b of g/r is at 0123456.");
});

test("branch_read leaves out what GitLab did not give, and refuses a missing branch plainly", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { name: "x", commit: { id: SHA } }), reply(404, { message: "404 Branch Not Found" }));

  const { payload } = await branchRead({ repo: "g/r", branch: "x" });
  assert.equal("message" in payload, false);
  assert.equal("date" in payload, false);
  assert.equal(payload.protected, false);
  expectOutput(read, payload);

  await assert.rejects(
    branchRead({ repo: "g/r", branch: "nope" }),
    isRefusal(/^GitLab could not find what it needs to read the branch 'nope' of g\/r \(404\): 404 Branch Not Found\./),
  );
  await assert.rejects(branchRead({ repo: "g/r", branch: "a b" }), isRefusal(/`branch` 'a b' is not a valid/));
});

/* ------------------------------------------------------------------ branch_list */

test("branch_list sends the search and the paging, and lists each branch", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, [branch(), branch({ name: "feat/x", protected: false, default: false, merged: true, commit: { id: OTHER } })], {
      "x-next-page": "3",
    }),
  );

  const { payload } = await branchList({ repo: "g/r", search: " ^feat ", page: 2, per_page: 2 });

  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/api/v4/projects/g%2Fr/repository/branches");
  assert.deepEqual(Object.fromEntries(url.searchParams), { search: "^feat", page: "2", per_page: "2" });
  assert.deepEqual(payload, {
    summary: "2 branches matching '^feat' of g/r on page 2.",
    items: [
      { name: "main", sha: SHA, protected: true, default: true, merged: false },
      { name: "feat/x", sha: OTHER, protected: false, default: false, merged: true },
    ],
    page: 2,
    per_page: 2,
    has_more: true,
  });
  expectOutput(list, payload);
});

test("branch_list sends no search by default, and reports an empty page", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, []), reply(200, [branch()]));

  const empty = await branchList({ repo: "g/r", page: 9 });
  assert.equal(new URL(calls[0].url).searchParams.has("search"), false);
  assert.equal(empty.payload.summary, "g/r has no branches on page 9.");
  assert.equal(empty.payload.has_more, false);

  const one = await branchList({ repo: "g/r" });
  assert.equal(one.payload.summary, "1 branch of g/r on page 1.");
});

test("branch_list refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, []));

  await assert.rejects(branchList({ repo: "g/r", search: 5 }), isRefusal(/`search` must be a string/));
  await assert.rejects(branchList({ repo: "g/r", per_page: 101 }), isRefusal(/`per_page` must be between 1 and 100/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ branch_delete */

test("branch_delete removes the branch and says what it removed", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204));

  const { payload } = await branchDelete({ repo: "g/r", branch: "feat/x" });

  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/branches/feat%2Fx`);
  assert.deepEqual(payload, { summary: "Deleted branch feat/x from g/r.", deleted: true, branch: "feat/x" });
  expectOutput(del, payload);
});

test("branch_delete explains a refusal as a missing, default or protected branch", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "404 Branch Not Found" }), reply(403, { message: "403 Forbidden" }));

  await assert.rejects(
    branchDelete({ repo: "g/r", branch: "main" }),
    isRefusal(/^GitLab would not delete the branch 'main' from g\/r \(404\): 404 Branch Not Found\. The branch may not exist, or it is the default branch or a protected branch/),
  );
  await assert.rejects(branchDelete({ repo: "g/r", branch: "main" }), isRefusal(/\(403\): 403 Forbidden\./));
});

test("branch_delete refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204));

  await assert.rejects(branchDelete({ repo: "g/r" }), isRefusal(/`branch` is required/));
  await assert.rejects(branchDelete({ repo: "g/r", branch: ".hidden" }), isRefusal(/not a valid branch/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ branch_merge */

/** A merge request as GitLab returns it. */
const request = (overrides = {}) => ({
  id: 900,
  iid: 12,
  title: "Merge feat/x into main",
  state: "opened",
  sha: SHA,
  detailed_merge_status: "mergeable",
  merge_status: "can_be_merged",
  web_url: "https://gitlab.com/g/r/-/merge_requests/12",
  ...overrides,
});

const ahead = { commits: [{ id: OTHER }], diffs: [{}], compare_same_ref: false };

test("branch_merge compares, opens a merge request, accepts it, and reports the merge commit", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, ahead),
    reply(201, request()),
    reply(200, request({ state: "merged", merge_commit_sha: OTHER })),
  );

  const { payload, ok } = await branchMerge({ repo: "g/r", base: "main", head: "feat/x", message: " Ship it " });

  assert.equal(ok, true);
  assert.equal(calls.length, 3);
  const compare = new URL(calls[0].url);
  assert.equal(compare.pathname, "/api/v4/projects/g%2Fr/repository/compare");
  assert.deepEqual(Object.fromEntries(compare.searchParams), { from: "main", to: "feat/x" });
  assert.equal(calls[1].method, "POST");
  assert.equal(calls[1].url, `${API}/projects/g%2Fr/merge_requests`);
  assert.deepEqual(calls[1].body, {
    source_branch: "feat/x",
    target_branch: "main",
    title: "Merge feat/x into main",
    remove_source_branch: false,
  });
  assert.equal(calls[2].method, "PUT");
  assert.equal(calls[2].url, `${API}/projects/g%2Fr/merge_requests/12/merge`);
  assert.deepEqual(calls[2].body, { sha: SHA, should_remove_source_branch: false, merge_commit_message: "Ship it" });
  assert.deepEqual(payload, {
    summary: "Merged feat/x into main in g/r, at fedcba9, through merge request !12.",
    merged: true,
    up_to_date: false,
    sha: OTHER,
    merge_request: 12,
    html_url: "https://gitlab.com/g/r/-/merge_requests/12",
  });
  expectOutput(merge, payload);
});

test("branch_merge sends no message by default, and opens no merge request when the base already has the head", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { commits: [], diffs: [], compare_same_ref: false }));

  const { payload, ok } = await branchMerge({ repo: "g/r", base: "main", head: "feat/x" });

  assert.equal(ok, true);
  assert.equal(calls.length, 1, "only the comparison was sent");
  assert.deepEqual(payload, { summary: "Nothing to merge: main already contains feat/x.", merged: false, up_to_date: true });
  expectOutput(merge, payload);
});

test("branch_merge waits while GitLab works out whether it can merge, then merges", async (t) => {
  withToken(t);
  const waited = [];
  t.mock.method(globalThis, "setTimeout", (fn, ms) => {
    waited.push(ms);
    fn();
    return 0;
  });
  const { calls } = mockApi(
    t,
    reply(200, ahead),
    reply(201, request({ detailed_merge_status: "checking", merge_status: "checking" })),
    reply(200, request({ detailed_merge_status: "checking" })),
    reply(200, request()),
    reply(200, request({ state: "merged", merge_commit_sha: OTHER })),
  );

  const { payload } = await branchMerge({ repo: "g/r", base: "main", head: "feat/x" });

  assert.deepEqual(calls.map((call) => `${call.method} ${call.url.replace(API, "").split("?")[0]}`), [
    "GET /projects/g%2Fr/repository/compare",
    "POST /projects/g%2Fr/merge_requests",
    "GET /projects/g%2Fr/merge_requests/12",
    "GET /projects/g%2Fr/merge_requests/12",
    "PUT /projects/g%2Fr/merge_requests/12/merge",
  ]);
  assert.deepEqual(waited, [1000, 1000]);
  assert.equal(payload.merged, true);
});

test("branch_merge closes the request it opened, and says why, when the branches conflict", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, ahead),
    reply(201, request({ detailed_merge_status: "conflict", merge_status: "cannot_be_merged" })),
    reply(200, request({ state: "closed" })),
  );

  await assert.rejects(
    branchMerge({ repo: "g/r", base: "main", head: "feat/x" }),
    isRefusal(/^GitLab will not merge 'feat\/x' into 'main' in g\/r: the branches conflict\. The merge request it opened, !12, was closed again\.$/),
  );
  assert.equal(calls.length, 3);
  assert.equal(calls[2].method, "PUT");
  assert.equal(calls[2].url, `${API}/projects/g%2Fr/merge_requests/12`);
  assert.deepEqual(calls[2].body, { state_event: "close" });
});

test("branch_merge closes the request when the merge itself is refused, and names the cause", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, ahead),
    reply(201, request()),
    reply(405, { message: "405 Method Not Allowed" }),
    reply(200, request({ state: "closed" })),
    reply(200, ahead),
    reply(201, request()),
    reply(409, { message: "SHA does not match HEAD of source branch" }),
    reply(200, request({ state: "closed" })),
    reply(200, ahead),
    reply(201, request()),
    reply(401, { message: "401 Unauthorized" }),
    reply(500, { message: "boom" }),
  );

  await assert.rejects(
    branchMerge({ repo: "g/r", base: "main", head: "feat/x" }),
    isRefusal(/could not merge 'feat\/x' into 'main' in g\/r \(405\): 405 Method Not Allowed\. It cannot be merged as it stands\. The merge request it opened, !12, was closed again\.$/),
  );
  await assert.rejects(
    branchMerge({ repo: "g/r", base: "main", head: "feat/x" }),
    isRefusal(/\(409\): SHA does not match HEAD of source branch\. 'feat\/x' moved while the merge was being made; try again\. The merge request it opened, !12, was closed again\.$/),
  );
  await assert.rejects(
    branchMerge({ repo: "g/r", base: "main", head: "feat/x" }),
    isRefusal(/\(401\).*Maintainer role.* The merge request it opened, !12, is still open; close it with pr_close\.$/),
  );
  assert.equal(calls.filter((call) => call.method === "PUT" && call.url.endsWith("/merge")).length, 3);
});

test("branch_merge gives up when GitLab never finishes working it out, and closes the request", async (t) => {
  withToken(t);
  t.mock.method(globalThis, "setTimeout", (fn) => {
    fn();
    return 0;
  });
  const checking = request({ detailed_merge_status: "checking" });
  const { calls } = mockApi(t, reply(200, ahead), reply(201, checking), ...Array.from({ length: 10 }, () => reply(200, checking)), reply(200, request({ state: "closed" })));

  await assert.rejects(
    branchMerge({ repo: "g/r", base: "main", head: "feat/x" }),
    isRefusal(/^GitLab is still working out whether 'feat\/x' can be merged into 'main' in g\/r\. The merge request it opened, !12, was closed again\. Try again in a moment\.$/),
  );
  assert.equal(calls.length, 13);
});

test("branch_merge reports a merge GitLab is still completing, and one it did not complete", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, ahead),
    reply(201, request()),
    reply(200, request({ state: "locked" })),
    reply(200, ahead),
    reply(201, request()),
    reply(200, request({ state: "opened" })),
  );

  const locked = await branchMerge({ repo: "g/r", base: "main", head: "feat/x" });
  assert.equal(locked.ok, true);
  assert.equal(locked.payload.merged, false);
  assert.match(locked.payload.summary, /is completing it; read it with pr_read\.$/);
  expectOutput(merge, locked.payload);

  const open = await branchMerge({ repo: "g/r", base: "main", head: "feat/x" });
  assert.equal(open.ok, false);
  assert.match(open.payload.summary, /without saying it merged \(merge request !12 is opened\)/);
});

test("branch_merge explains a missing branch and a merge request that is already open", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "404 Compare Not Found" }), reply(200, ahead), reply(409, { message: ["Another open merge request already exists for this source branch: !5"] }));

  await assert.rejects(
    branchMerge({ repo: "g/r", base: "main", head: "ghost" }),
    isRefusal(/could not find 'main' or 'ghost' in g\/r \(404\).*Check both names/),
  );
  await assert.rejects(
    branchMerge({ repo: "g/r", base: "main", head: "feat/x" }),
    isRefusal(/could not open a merge request from 'feat\/x' into 'main' in g\/r \(409\): Another open merge request already exists for this source branch: !5\. One is already open; merge that one with pr_merge\./),
  );
});

test("branch_merge refuses a body that is not a merge request, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, ahead), reply(201, { nope: true }));

  await assert.rejects(branchMerge({ repo: "g/r", head: "x" }), isRefusal(/`base` is required/));
  await assert.rejects(branchMerge({ repo: "g/r", base: "main" }), isRefusal(/`head` is required/));
  await assert.rejects(branchMerge({ repo: "g/r", base: "a b", head: "x" }), isRefusal(/`base` 'a b' is not a valid/));
  await assert.rejects(branchMerge({ repo: "g/r", base: "main", head: "x", message: 4 }), isRefusal(/`message` must be a string/));
  assert.equal(calls.length, 0);

  await assert.rejects(branchMerge({ repo: "g/r", base: "main", head: "x" }), isRefusal(/not a merge request this server recognizes/));
});

test("mergeability reads the detailed status, falls back to the older one, and says why a request is not ready", () => {
  assert.deepEqual(mergeability({ detailed_merge_status: "mergeable" }), { status: "mergeable", ready: true, pending: false, reason: "GitLab reports it as mergeable" });
  for (const waiting of ["checking", "unchecked", "preparing"]) assert.equal(mergeability({ detailed_merge_status: waiting }).pending, true, waiting);
  assert.equal(mergeability({ detailed_merge_status: "ci_must_pass" }).reason, "its pipeline has to pass first");
  assert.equal(mergeability({ detailed_merge_status: "draft_status" }).ready, false);
  assert.equal(mergeability({ detailed_merge_status: "something_new" }).reason, "GitLab reports it as something_new");
  assert.equal(mergeability({ merge_status: "can_be_merged" }).ready, true);
  assert.equal(mergeability({ merge_status: "cannot_be_merged" }).ready, false);
  assert.equal(mergeability({ merge_status: "checking" }).pending, true);
  assert.equal(mergeability({}).pending, true);
});
