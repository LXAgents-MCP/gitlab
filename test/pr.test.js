import assert from "node:assert/strict";
import test from "node:test";
import { TOOL as approve, handle as prApprove } from "../src/tools/pr-approve.js";
import { TOOL as close, handle as prClose } from "../src/tools/pr-close.js";
import { TOOL as comment, handle as prComment } from "../src/tools/pr-comment.js";
import { TOOL as merge, handle as prMerge } from "../src/tools/pr-merge.js";
import { TOOL as open, handle as prOpen } from "../src/tools/pr-open.js";
import { TOOL as read, handle as prRead } from "../src/tools/pr-read.js";
import { TOOL as request, handle as prReviewRequest } from "../src/tools/pr-review-request.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const API = "https://gitlab.com/api/v4";
const BASE = `${API}/projects/g%2Fr/merge_requests/7`;
const SHA = "0123456789abcdef0123456789abcdef01234567";
const MERGE_SHA = "fedcba9876543210fedcba9876543210fedcba98";

const user = (id, username) => ({ id, username, name: username, state: "active" });

/** A merge request as GitLab returns it. */
const mr = (overrides = {}) => ({
  id: 9001,
  iid: 7,
  project_id: 3,
  title: "Add login",
  description: "Adds the login page.",
  state: "opened",
  draft: false,
  created_at: "2026-10-01T10:00:00.000Z",
  updated_at: "2026-10-02T10:00:00.000Z",
  merged_at: null,
  source_branch: "feat/login",
  target_branch: "main",
  sha: SHA,
  labels: ["ui"],
  assignees: [user(11, "alice")],
  reviewers: [user(13, "carol")],
  author: user(12, "bob"),
  user_notes_count: 2,
  changes_count: "3",
  merge_status: "can_be_merged",
  detailed_merge_status: "mergeable",
  web_url: "https://gitlab.com/g/r/-/merge_requests/7",
  ...overrides,
});

const note = (id, body, overrides = {}) => ({
  id,
  body,
  author: user(12, "bob"),
  created_at: "2026-10-01T11:00:00.000Z",
  system: false,
  ...overrides,
});

/* ------------------------------------------------------------------ pr_open */

test("pr_open opens a merge request into the default branch, as a draft when asked", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { default_branch: "main" }), reply(201, mr({ title: "Draft: Add login", draft: true })));

  const { payload, ok } = await prOpen({ repo: "g/r", title: " Add login ", head: "feat/login", body: "Adds the login page.", draft: true });

  assert.equal(ok, true);
  assert.equal(calls[0].url, `${API}/projects/g%2Fr`);
  assert.equal(calls[1].url, `${API}/projects/g%2Fr/merge_requests`);
  assert.equal(calls[1].method, "POST");
  assert.deepEqual(calls[1].body, {
    source_branch: "feat/login",
    target_branch: "main",
    title: "Draft: Add login",
    description: "Adds the login page.",
  });
  assert.deepEqual(payload, {
    summary: "Opened draft merge request !7 in g/r: feat/login into main.",
    number: 7,
    title: "Draft: Add login",
    state: "opened",
    draft: true,
    head: "feat/login",
    base: "main",
    html_url: "https://gitlab.com/g/r/-/merge_requests/7",
  });
  expectOutput(open, payload);
});

test("pr_open names the base it was given, looks nothing up, and does not mark a draft twice", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, mr()), reply(201, mr({ title: "Draft: Add login", draft: true })));

  const plain = await prOpen({ repo: "g/r", title: "Add login", head: "feat/login", base: "develop" });
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].body, { source_branch: "feat/login", target_branch: "develop", title: "Add login" });
  assert.equal(plain.payload.draft, false);
  assert.equal(plain.payload.base, "develop");

  await prOpen({ repo: "g/r", title: "Draft: Add login", head: "feat/login", base: "develop", draft: true });
  assert.equal(calls[1].body.title, "Draft: Add login");
});

test("pr_open says when one is already open, and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(409, { message: ["Another open merge request already exists for this source branch: !5"] }));

  await assert.rejects(prOpen({ repo: "g/r", head: "a" }), isRefusal(/`title` is required/));
  await assert.rejects(prOpen({ repo: "g/r", title: "x" }), isRefusal(/`head` is required/));
  await assert.rejects(prOpen({ repo: "g/r", title: "x", head: "a..b" }), isRefusal(/`head` 'a\.\.b' is not a valid/));
  await assert.rejects(prOpen({ repo: "g/r", title: "x", head: "a", base: "b c" }), isRefusal(/`base` 'b c' is not a valid/));
  await assert.rejects(prOpen({ repo: "g/r", title: "x", head: "a", draft: "yes" }), isRefusal(/`draft` must be true or false/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    prOpen({ repo: "g/r", title: "x", head: "a", base: "main" }),
    isRefusal(/would not open a merge request from 'a' into 'main' in g\/r \(409\): Another open merge request already exists.*One is already open for that branch; find it with pr_read/),
  );
});

/* ------------------------------------------------------------------ pr_read */

test("pr_read reports a merge request that can be merged", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, mr()));

  const { payload, ok } = await prRead({ repo: "g/r", number: 7 });

  assert.equal(ok, true);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, BASE);
  assert.deepEqual(payload, {
    summary: "Merge request !7 in g/r is opened: Add login (feat/login into main).",
    number: 7,
    title: "Add login",
    state: "opened",
    draft: false,
    merged: false,
    mergeable: true,
    merge_status: "mergeable",
    author: "bob",
    head: "feat/login",
    base: "main",
    head_sha: SHA,
    changed_files: "3",
    labels: ["ui"],
    assignees: ["alice"],
    reviewers: ["carol"],
    comment_count: 2,
    html_url: "https://gitlab.com/g/r/-/merge_requests/7",
    created_at: "2026-10-01T10:00:00.000Z",
    updated_at: "2026-10-02T10:00:00.000Z",
    body: "Adds the login page.",
  });
  expectOutput(read, payload);
});

test("pr_read says why a merge request cannot be merged yet, and when GitLab has not worked it out", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, mr({ draft: true, detailed_merge_status: "draft_status", merge_status: "cannot_be_merged" })),
    reply(200, mr({ detailed_merge_status: "checking", merge_status: "checking", changes_count: null })),
  );

  const blocked = await prRead({ repo: "g/r", number: 7 });
  assert.equal(blocked.payload.mergeable, false);
  assert.equal(blocked.payload.merge_status, "draft_status");
  assert.equal(blocked.payload.draft, true);
  assert.match(blocked.payload.summary, /is a draft, opened: Add login .* It cannot be merged yet: it is a draft\.$/);
  expectOutput(read, blocked.payload);

  const pending = await prRead({ repo: "g/r", number: 7 });
  assert.equal("mergeable" in pending.payload, false, "not known yet is not a no");
  assert.equal("changed_files" in pending.payload, false);
  assert.match(pending.payload.summary, /still being worked out; ask again in a moment\.$/);
  expectOutput(read, pending.payload);
});

test("pr_read reads a merged request without a verdict, and falls back to the older merge status", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, mr({ state: "merged", merged_at: "2026-10-03T10:00:00.000Z", detailed_merge_status: "not_open" })),
    reply(200, (({ detailed_merge_status, ...rest }) => rest)(mr({ merge_status: "can_be_merged" }))),
  );

  const merged = await prRead({ repo: "g/r", number: 7 });
  assert.equal(merged.payload.merged, true);
  assert.equal(merged.payload.merged_at, "2026-10-03T10:00:00.000Z");
  assert.equal("mergeable" in merged.payload, false);
  assert.match(merged.payload.summary, /is merged: Add login/);

  const older = await prRead({ repo: "g/r", number: 7 });
  assert.equal(older.payload.mergeable, true);
  assert.equal(older.payload.merge_status, "can_be_merged");
});

test("pr_read lists the changed files with line counts, and the comments people wrote", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, mr({ user_notes_count: 3 })),
    reply(200, [
      { old_path: "a.js", new_path: "a.js", new_file: false, renamed_file: false, deleted_file: false, diff: "@@ -1,2 +1,3 @@\n a\n-b\n+c\n+d\n" },
      { old_path: "b.js", new_path: "b.js", new_file: true, renamed_file: false, deleted_file: false, diff: "@@ -0,0 +1 @@\n+x\n" },
    ]),
    reply(200, [note(1, "first"), note(2, "approved this merge request", { system: true }), note(3, "second", { author: null })]),
  );

  const { payload } = await prRead({ repo: "g/r", number: 7, files: true, comments: true });

  const filesUrl = new URL(calls[1].url);
  assert.equal(filesUrl.pathname, "/api/v4/projects/g%2Fr/merge_requests/7/diffs");
  assert.equal(filesUrl.searchParams.get("per_page"), "100");
  const notesUrl = new URL(calls[2].url);
  assert.equal(notesUrl.pathname, "/api/v4/projects/g%2Fr/merge_requests/7/notes");
  assert.equal(notesUrl.searchParams.get("activity_filter"), "only_comments");
  assert.equal(notesUrl.searchParams.get("sort"), "asc");
  assert.deepEqual(payload.files, [
    { filename: "a.js", status: "modified", additions: 2, deletions: 1 },
    { filename: "b.js", status: "added", additions: 1, deletions: 0 },
  ]);
  assert.deepEqual(payload.comments, [
    { id: 1, author: "bob", created_at: "2026-10-01T11:00:00.000Z", body: "first" },
    { id: 3, created_at: "2026-10-01T11:00:00.000Z", body: "second" },
  ]);
  assert.match(payload.summary, /Showing the first 2 comments\.$/);
  expectOutput(read, payload);
});

test("pr_read asks for no comments a merge request does not have, and refuses what is not one", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, mr({ user_notes_count: 0 })), reply(200, "<html>"), reply(404, { message: "404 Not found" }));

  await assert.rejects(prRead({ repo: "g/r", number: 0 }), isRefusal(/`number` must be between 1/));
  await assert.rejects(prRead({ repo: "g/r" }), isRefusal(/`number` is required/));
  await assert.rejects(prRead({ repo: "g/r", number: 7, files: 1 }), isRefusal(/`files` must be true or false/));
  assert.equal(calls.length, 0);

  const { payload } = await prRead({ repo: "g/r", number: 7, comments: true });
  assert.equal(calls.length, 1);
  assert.equal("comments" in payload, false);

  await assert.rejects(prRead({ repo: "g/r", number: 7 }), isRefusal(/body was not a merge request this server recognizes/));
  await assert.rejects(prRead({ repo: "g/r", number: 99 }), isRefusal(/could not find what it needs to read merge request !99 of g\/r \(404\)/));
});

/* ------------------------------------------------------------------ pr_comment */

test("pr_comment adds a comment and links to it", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, note(55, "Looks good")), reply(201, note(56, "x")), reply(200, { unexpected: true }));

  const { payload, ok } = await prComment({ repo: "g/r", number: 7, body: " Looks good\n" });

  assert.equal(ok, true);
  assert.equal(calls[0].url, `${BASE}/notes`);
  assert.equal(calls[0].method, "POST");
  assert.deepEqual(calls[0].body, { body: " Looks good\n" });
  assert.deepEqual(payload, {
    summary: "Commented on merge request !7 in g/r.",
    id: 55,
    html_url: "https://gitlab.com/g/r/-/merge_requests/7#note_55",
  });
  expectOutput(comment, payload);

  const byId = await prComment({ repo: "3", number: 7, body: "x" });
  assert.equal("html_url" in byId.payload, false);

  await assert.rejects(prComment({ repo: "g/r", number: 7, body: "x" }), isRefusal(/body was not a comment this server recognizes/));
});

test("pr_comment refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, note(1, "x")));

  await assert.rejects(prComment({ repo: "g/r", number: 7 }), isRefusal(/`body` is required/));
  await assert.rejects(prComment({ repo: "g/r", number: 7, body: "  " }), isRefusal(/`body` is required and must not be blank/));
  await assert.rejects(prComment({ repo: "g/r", body: "x" }), isRefusal(/`number` is required/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ pr_close */

test("pr_close comments first, then closes", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, note(1, "Superseded")), reply(200, mr({ state: "closed" })));

  const { payload, ok } = await prClose({ repo: "g/r", number: 7, comment: "Superseded" });

  assert.equal(ok, true);
  assert.equal(calls[0].url, `${BASE}/notes`);
  assert.deepEqual(calls[0].body, { body: "Superseded" });
  assert.equal(calls[1].url, BASE);
  assert.equal(calls[1].method, "PUT");
  assert.deepEqual(calls[1].body, { state_event: "close" });
  assert.deepEqual(payload, {
    summary: "Closed merge request !7 in g/r without merging.",
    number: 7,
    state: "closed",
    merged: false,
    html_url: "https://gitlab.com/g/r/-/merge_requests/7",
  });
  expectOutput(close, payload);
});

test("pr_close without a comment makes one request, and a blank comment is none", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, mr({ state: "closed" })), reply(200, mr({ state: "closed" })));

  await prClose({ repo: "g/r", number: 7 });
  await prClose({ repo: "g/r", number: 7, comment: "  " });

  assert.deepEqual(calls.map((call) => call.method), ["PUT", "PUT"]);
});

test("pr_close says a merged request cannot be closed, that the comment was posted, and what GitLab left open", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(201, note(1, "why")),
    reply(405, { message: "405 Method Not Allowed" }),
    reply(200, mr({ state: "merged" })),
  );

  await assert.rejects(
    prClose({ repo: "g/r", number: 7, comment: "why" }),
    isRefusal(/would not close merge request !7 of g\/r \(405\): 405 Method Not Allowed\. A merge request that is already merged cannot be closed.*The comment had already been posted\.$/),
  );

  const { payload } = await prClose({ repo: "g/r", number: 7 });
  assert.equal(payload.summary, "GitLab answered, but merge request !7 in g/r is merged.");
  assert.equal(payload.merged, true);
  expectOutput(close, payload);
});

test("pr_close refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(prClose({ repo: "g/r" }), isRefusal(/`number` is required/));
  await assert.rejects(prClose({ repo: "g/r", number: 7, comment: 4 }), isRefusal(/`comment` must be a string/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ pr_merge */

test("pr_merge merges, and reports the commit it made", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, mr({ state: "merged", merge_commit_sha: MERGE_SHA, squash_commit_sha: null })));

  const { payload, ok } = await prMerge({ repo: "g/r", number: 7, message: "Merge login", sha: SHA });

  assert.equal(ok, true);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, `${BASE}/merge`);
  assert.equal(calls[0].method, "PUT");
  assert.deepEqual(calls[0].body, { merge_commit_message: "Merge login", sha: SHA });
  assert.deepEqual(payload, {
    summary: "Merged merge request !7 in g/r, at fedcba9.",
    merged: true,
    state: "merged",
    sha: MERGE_SHA,
  });
  expectOutput(merge, payload);
});

test("pr_merge squashes on request, and sends the message to the squash commit", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, mr({ state: "merged", merge_commit_sha: null, squash_commit_sha: MERGE_SHA })), reply(200, mr({ state: "merged" })));

  const squashed = await prMerge({ repo: "g/r", number: 7, squash: true, message: "Add login" });
  assert.deepEqual(calls[0].body, { squash: true, squash_commit_message: "Add login" });
  assert.equal(squashed.payload.sha, MERGE_SHA);

  const plain = await prMerge({ repo: "g/r", number: 7, squash: false });
  assert.deepEqual(calls[1].body, { squash: false });
  assert.equal("sha" in plain.payload, false, "a fast-forward makes no commit of its own");
  assert.match(plain.payload.summary, /^Merged merge request !7 in g\/r\.$/);
});

test("pr_merge says when GitLab is still completing the merge, and when it answers without merging", async (t) => {
  withToken(t);
  mockApi(t, reply(200, mr({ state: "locked" })), reply(200, mr({ state: "opened" })));

  const locked = await prMerge({ repo: "g/r", number: 7 });
  assert.equal(locked.ok, true);
  assert.equal(locked.payload.merged, false);
  assert.match(locked.payload.summary, /is completing it; read it with pr_read\./);
  expectOutput(merge, locked.payload);

  const open = await prMerge({ repo: "g/r", number: 7 });
  assert.equal(open.ok, false);
  assert.match(open.payload.summary, /without saying it merged \(it is opened\)/);
});

test("pr_merge asks GitLab why when it will not merge", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(405, { message: "405 Method Not Allowed" }),
    reply(200, mr({ draft: true, detailed_merge_status: "draft_status" })),
    reply(422, { message: "Branch cannot be merged" }),
    reply(200, mr({ state: "closed" })),
    reply(406, { message: "Branch cannot be merged" }),
    reply(200, mr({ detailed_merge_status: "checking" })),
    reply(405, { message: "405 Method Not Allowed" }),
    reply(200, mr({ detailed_merge_status: "mergeable" })),
    reply(405, { message: "405 Method Not Allowed" }),
    reply(500, { message: "boom" }),
  );

  await assert.rejects(
    prMerge({ repo: "g/r", number: 7 }),
    isRefusal(/would not merge merge request !7 of g\/r \(405\): 405 Method Not Allowed\. It cannot be merged yet: it is a draft\.$/),
  );
  assert.equal(calls[1].method, "GET");
  assert.equal(calls[1].url, BASE);

  await assert.rejects(prMerge({ repo: "g/r", number: 7 }), isRefusal(/\(422\): Branch cannot be merged\. It is closed\.$/));
  await assert.rejects(prMerge({ repo: "g/r", number: 7 }), isRefusal(/\(406\).* still working out whether it can be merged; try again in a moment\.$/));
  await assert.rejects(prMerge({ repo: "g/r", number: 7 }), isRefusal(/GitLab reports it as mergeable, so the project's merge settings refused it\.$/));
  await assert.rejects(prMerge({ repo: "g/r", number: 7 }), isRefusal(/\(405\): 405 Method Not Allowed\.$/), "the reason is a courtesy");
});

test("pr_merge explains a missing permission and a branch that moved", async (t) => {
  withToken(t);
  mockApi(t, reply(401, { message: "401 Unauthorized" }), reply(409, { message: "SHA does not match HEAD of source branch" }));

  await assert.rejects(
    prMerge({ repo: "g/r", number: 7 }),
    isRefusal(/would not let this token merge !7 in g\/r \(401\): 401 Unauthorized\. Either the token is not valid, or its user may not merge.*Maintainer/),
  );
  await assert.rejects(
    prMerge({ repo: "g/r", number: 7, sha: SHA }),
    isRefusal(/did not merge !7 in g\/r \(409\): SHA does not match HEAD of source branch\. The branch moved/),
  );
});

test("pr_merge refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(prMerge({ repo: "g/r" }), isRefusal(/`number` is required/));
  await assert.rejects(prMerge({ repo: "g/r", number: 7, squash: "yes" }), isRefusal(/`squash` must be true or false/));
  await assert.rejects(prMerge({ repo: "g/r", number: 7, message: 3 }), isRefusal(/`message` must be a string/));
  await assert.rejects(prMerge({ repo: "g/r", number: 7, sha: 3 }), isRefusal(/`sha` must be a string/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ pr_review_request */

test("pr_review_request adds reviewers to those already asked", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, mr({ reviewers: [user(13, "carol")] })),
    reply(200, [user(14, "dave")]),
    reply(200, mr({ reviewers: [user(13, "carol"), user(14, "dave")] })),
  );

  const { payload, ok } = await prReviewRequest({ repo: "g/r", number: 7, reviewers: ["@dave"] });

  assert.equal(ok, true);
  assert.equal(calls[0].url, BASE);
  assert.equal(calls[1].url, `${API}/users?username=dave`);
  assert.equal(calls[2].method, "PUT");
  assert.deepEqual(calls[2].body, { reviewer_ids: [13, 14] });
  assert.deepEqual(payload, { summary: "Review requested on !7 in g/r from: carol, dave.", number: 7, reviewers: ["carol", "dave"] });
  expectOutput(request, payload);
});

test("pr_review_request does not write when they were all asked already, and says who GitLab did not add", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, mr()),
    reply(200, [user(13, "Carol")]),
    reply(200, mr({ reviewers: [] })),
    reply(200, [user(14, "dave")]),
    reply(200, mr({ reviewers: [] })),
  );

  const again = await prReviewRequest({ repo: "g/r", number: 7, reviewers: ["carol"] });
  assert.deepEqual(calls.map((call) => call.method), ["GET", "GET"]);
  assert.deepEqual(again.payload.reviewers, ["carol"]);

  const dropped = await prReviewRequest({ repo: "g/r", number: 7, reviewers: ["dave"] });
  assert.deepEqual(dropped.payload.not_applied, ["dave"]);
  assert.match(dropped.payload.summary, /from: nobody\. GitLab did not add: dave\./);
  expectOutput(request, dropped.payload);
});

test("pr_review_request refuses an unknown user before changing anything, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, mr()), reply(200, []));

  await assert.rejects(prReviewRequest({ repo: "g/r", number: 7 }), isRefusal(/`reviewers` is required/));
  await assert.rejects(prReviewRequest({ repo: "g/r", number: 7, reviewers: [] }), isRefusal(/`reviewers` is required and must list at least one/));
  await assert.rejects(prReviewRequest({ repo: "g/r", reviewers: ["a"] }), isRefusal(/`number` is required/));
  assert.equal(calls.length, 0);

  await assert.rejects(prReviewRequest({ repo: "g/r", number: 7, reviewers: ["ghost"] }), isRefusal(/GitLab has no user named 'ghost', so nothing was changed/));
  assert.ok(calls.every((call) => call.method === "GET"));
});

/* ------------------------------------------------------------------ pr_approve */

test("pr_approve records the approval and says how many are still needed", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(201, { iid: 7, approvals_required: 2, approvals_left: 1, approved_by: [{ user: user(11, "alice") }] }),
    reply(201, { iid: 7, approvals_left: 0, approved_by: [{ user: user(11, "alice") }, { user: user(13, "carol") }] }),
    reply(201, { iid: 7, approved_by: [] }),
  );

  const first = await prApprove({ repo: "g/r", number: 7, sha: SHA });
  assert.equal(calls[0].url, `${BASE}/approve`);
  assert.equal(calls[0].method, "POST");
  assert.deepEqual(calls[0].body, { sha: SHA });
  assert.deepEqual(first.payload, {
    summary: "Approved merge request !7 in g/r; approved by alice, 1 more needed.",
    number: 7,
    approved_by: ["alice"],
    approvals_left: 1,
  });
  expectOutput(approve, first.payload);

  const second = await prApprove({ repo: "g/r", number: 7 });
  assert.equal(calls[1].body, undefined, "no sha, no body");
  assert.match(second.payload.summary, /approved by alice, carol, no approvals left to wait for\.$/);

  const bare = await prApprove({ repo: "g/r", number: 7 });
  assert.equal("approvals_left" in bare.payload, false);
  assert.match(bare.payload.summary, /approved by nobody listed\.$/);
  expectOutput(approve, bare.payload);
});

test("pr_approve explains a refusal, and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(401, { message: "401 Unauthorized" }),
    reply(409, { message: "SHA does not match HEAD of source branch" }),
    reply(201, { unexpected: true }),
  );

  await assert.rejects(prApprove({ repo: "g/r" }), isRefusal(/`number` is required/));
  await assert.rejects(prApprove({ repo: "g/r", number: 7, sha: 3 }), isRefusal(/`sha` must be a string/));
  assert.equal(calls.length, 0);

  await assert.rejects(prApprove({ repo: "g/r", number: 7 }), isRefusal(/would not record the approval on !7 of g\/r \(401\).*forbid approving your own/));
  await assert.rejects(prApprove({ repo: "g/r", number: 7, sha: SHA }), isRefusal(/did not record the approval on !7 of g\/r \(409\)/));
  await assert.rejects(prApprove({ repo: "g/r", number: 7 }), isRefusal(/body was not an approval this server recognizes/));
});
