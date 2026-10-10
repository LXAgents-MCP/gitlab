import assert from "node:assert/strict";
import test from "node:test";
import { labelList } from "../src/args.js";
import { TOOL as assign, handle as issueAssign } from "../src/tools/issue-assign.js";
import { TOOL as close, handle as issueClose } from "../src/tools/issue-close.js";
import { TOOL as comment, handle as issueComment } from "../src/tools/issue-comment.js";
import { TOOL as label, handle as issueLabel } from "../src/tools/issue-label.js";
import { TOOL as lock, handle as issueLock } from "../src/tools/issue-lock.js";
import { TOOL as open, handle as issueOpen } from "../src/tools/issue-open.js";
import { TOOL as read, handle as issueRead } from "../src/tools/issue-read.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const API = "https://gitlab.com/api/v4";
const BASE = `${API}/projects/g%2Fr/issues/7`;

const user = (id, username) => ({ id, username, name: username, state: "active" });

/** An issue as GitLab returns it. */
const issue = (overrides = {}) => ({
  id: 9001,
  iid: 7,
  project_id: 3,
  title: "It crashes",
  description: "Steps:\n\n1. run it",
  state: "opened",
  created_at: "2026-10-01T10:00:00.000Z",
  updated_at: "2026-10-02T10:00:00.000Z",
  closed_at: null,
  labels: ["bug", "p1"],
  milestone: { id: 4, iid: 1, title: "v1" },
  assignees: [user(11, "alice")],
  author: user(12, "bob"),
  user_notes_count: 2,
  confidential: false,
  discussion_locked: null,
  web_url: "https://gitlab.com/g/r/-/issues/7",
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

/* ------------------------------------------------------------------ issue_open */

test("issue_open opens an issue with labels, assignees and a milestone, and reports what GitLab applied", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [user(11, "alice")]), reply(201, issue()));

  const { payload, ok } = await issueOpen({
    repo: "g/r",
    title: " It crashes ",
    body: "Steps:\n\n1. run it",
    labels: ["bug", "p1"],
    assignees: ["@alice"],
    milestone: 4,
  });

  assert.equal(ok, true);
  assert.equal(calls[0].url, `${API}/users?username=alice`);
  assert.equal(calls[0].method, "GET");
  assert.equal(calls[1].url, `${API}/projects/g%2Fr/issues`);
  assert.equal(calls[1].method, "POST");
  assert.deepEqual(calls[1].body, {
    title: "It crashes",
    description: "Steps:\n\n1. run it",
    labels: "bug,p1",
    assignee_ids: [11],
    milestone_id: 4,
  });
  assert.deepEqual(payload, {
    summary: "Opened issue #7 in g/r: It crashes.",
    number: 7,
    title: "It crashes",
    state: "opened",
    html_url: "https://gitlab.com/g/r/-/issues/7",
    labels: ["bug", "p1"],
    assignees: ["alice"],
  });
  expectOutput(open, payload);
});

test("issue_open sends only the title when that is all it was given, and looks nobody up", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, issue({ labels: [], assignees: [], milestone: null })));

  const { payload } = await issueOpen({ repo: "g/r", title: "Plain", labels: [], assignees: [] });

  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].body, { title: "Plain" });
  assert.deepEqual(payload.labels, []);
  assert.deepEqual(payload.assignees, []);
});

test("issue_open says what GitLab did not apply", async (t) => {
  withToken(t);
  mockApi(t, reply(200, [user(11, "alice")]), reply(200, [user(13, "carol")]), reply(201, issue({ assignees: [user(11, "alice")], labels: ["bug"] })));

  const { payload } = await issueOpen({ repo: "g/r", title: "x", labels: ["Bug", "p1"], assignees: ["alice", "carol"] });

  assert.match(payload.summary, /^Opened issue #7 in g\/r: It crashes\. GitLab did not apply: label p1, assignee carol\./);
  assert.match(payload.summary, /A free plan keeps one assignee/);
});

test("issue_open refuses an unknown username before it writes anything", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [user(11, "alice")]), reply(200, []));

  await assert.rejects(
    issueOpen({ repo: "g/r", title: "x", assignees: ["alice", "nobody"] }),
    isRefusal(/GitLab has no user named 'nobody', so nothing was changed/),
  );
  assert.equal(calls.length, 2);
  assert.ok(calls.every((call) => call.method === "GET"));
});

test("issue_open refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(issueOpen({ repo: "g/r" }), isRefusal(/`title` is required/));
  await assert.rejects(issueOpen({ repo: "g/r", title: "x", body: 3 }), isRefusal(/`body` must be a string/));
  await assert.rejects(issueOpen({ repo: "g/r", title: "x", labels: ["a,b"] }), isRefusal(/`labels` may not contain a comma.*'a,b'/));
  await assert.rejects(issueOpen({ repo: "g/r", title: "x", labels: "bug" }), isRefusal(/`labels` must be a list of strings/));
  await assert.rejects(issueOpen({ repo: "g/r", title: "x", assignees: Array.from({ length: 11 }, (_, i) => `u${i}`) }), isRefusal(/at most 10/));
  await assert.rejects(issueOpen({ repo: "g/r", title: "x", milestone: 0 }), isRefusal(/`milestone` must be between 1/));
  await assert.rejects(issueOpen({ repo: "bad", title: "x" }), isRefusal(/is not a project/));
  assert.equal(calls.length, 0);
  await assert.rejects(issueOpen({ repo: "g/r", title: "x" }), isRefusal(/MCP_AUTH_TOKEN is not set/));
});

test("labelList refuses a comma and passes everything else through", () => {
  assert.deepEqual(labelList([" a ", "b", "a"], "labels"), ["a", "b"]);
  assert.equal(labelList(undefined, "labels"), undefined);
  assert.throws(() => labelList(["ok", "no,pe"], "labels"), isRefusal(/'no,pe'/));
});

/* ------------------------------------------------------------------ issue_read */

test("issue_read reports an issue, and what changed on it when asked", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue()));

  const { payload, ok } = await issueRead({ repo: "g/r", number: 7 });

  assert.equal(ok, true);
  assert.equal(calls.length, 1, "no comments were asked for");
  assert.equal(calls[0].url, BASE);
  assert.deepEqual(payload, {
    summary: "Issue #7 in g/r is opened: It crashes.",
    number: 7,
    title: "It crashes",
    state: "opened",
    author: "bob",
    labels: ["bug", "p1"],
    assignees: ["alice"],
    milestone: "v1",
    comment_count: 2,
    locked: false,
    confidential: false,
    html_url: "https://gitlab.com/g/r/-/issues/7",
    created_at: "2026-10-01T10:00:00.000Z",
    updated_at: "2026-10-02T10:00:00.000Z",
    body: "Steps:\n\n1. run it",
  });
  expectOutput(read, payload);
});

test("issue_read returns the comments, only the ones people wrote, oldest first", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, issue({ user_notes_count: 3, discussion_locked: true, state: "closed", closed_at: "2026-10-03T10:00:00.000Z" })),
    reply(200, [note(1, "first"), note(2, "added label ~bug", { system: true }), note(3, "second", { author: null })]),
  );

  const { payload } = await issueRead({ repo: "g/r", number: 7, comments: true });

  const url = new URL(calls[1].url);
  assert.equal(url.pathname, "/api/v4/projects/g%2Fr/issues/7/notes");
  assert.equal(url.searchParams.get("activity_filter"), "only_comments");
  assert.equal(url.searchParams.get("sort"), "asc");
  assert.equal(url.searchParams.get("order_by"), "created_at");
  assert.equal(url.searchParams.get("per_page"), "30");
  assert.deepEqual(
    payload.comments,
    [
      { id: 1, author: "bob", created_at: "2026-10-01T11:00:00.000Z", body: "first" },
      { id: 3, created_at: "2026-10-01T11:00:00.000Z", body: "second" },
    ],
  );
  assert.equal(payload.locked, true);
  assert.equal(payload.closed_at, "2026-10-03T10:00:00.000Z");
  assert.match(payload.summary, /is closed: It crashes; showing the first 2 comments\./);
  expectOutput(read, payload);
});

test("issue_read does not ask for comments an issue does not have", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ user_notes_count: 0 })));

  const { payload } = await issueRead({ repo: "g/r", number: 7, comments: true });

  assert.equal(calls.length, 1);
  assert.equal("comments" in payload, false);
});

test("issue_read refuses what is not an issue, and bad arguments", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, "<html>"), reply(404, { message: "404 Not found" }));

  await assert.rejects(issueRead({ repo: "g/r", number: 0 }), isRefusal(/`number` must be between 1/));
  await assert.rejects(issueRead({ repo: "g/r" }), isRefusal(/`number` is required/));
  await assert.rejects(issueRead({ repo: "g/r", number: 7, comments: "yes" }), isRefusal(/`comments` must be true or false/));
  assert.equal(calls.length, 0);

  await assert.rejects(issueRead({ repo: "g/r", number: 7 }), isRefusal(/body was not an issue this server recognizes/));
  await assert.rejects(issueRead({ repo: "g/r", number: 99 }), isRefusal(/could not find what it needs to read issue #99 of g\/r \(404\)/));
});

/* ------------------------------------------------------------------ issue_comment */

test("issue_comment adds a comment and links to it", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, note(55, "Thanks")), reply(201, note(56, "Thanks")));

  const { payload, ok } = await issueComment({ repo: "g/r", number: 7, body: "  Thanks\n" });

  assert.equal(ok, true);
  assert.equal(calls[0].url, `${BASE}/notes`);
  assert.equal(calls[0].method, "POST");
  assert.deepEqual(calls[0].body, { body: "  Thanks\n" }, "the text is sent as written");
  assert.deepEqual(payload, {
    summary: "Commented on #7 in g/r.",
    id: 55,
    html_url: "https://gitlab.com/g/r/-/issues/7#note_55",
  });
  expectOutput(comment, payload);

  const byId = await issueComment({ repo: "3", number: 7, body: "x" });
  assert.equal("html_url" in byId.payload, false, "a project given by id has no path to build a link from");
  expectOutput(comment, byId.payload);
});

test("issue_comment explains a refusal, and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(403, { message: "403 Forbidden" }), reply(201, { unexpected: true }));

  await assert.rejects(issueComment({ repo: "g/r", number: 7 }), isRefusal(/`body` is required/));
  await assert.rejects(issueComment({ repo: "g/r", number: 7, body: "  " }), isRefusal(/`body` is required and must not be blank/));
  await assert.rejects(issueComment({ repo: "g/r", number: 1.5, body: "x" }), isRefusal(/`number` must be a whole number/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    issueComment({ repo: "g/r", number: 7, body: "x" }),
    isRefusal(/would not let this comment on #7 of g\/r \(403\): 403 Forbidden\. .*the discussion may be locked, in which case only project members can comment/),
  );
  await assert.rejects(issueComment({ repo: "g/r", number: 7, body: "x" }), isRefusal(/body was not a comment this server recognizes/));
});

/* ------------------------------------------------------------------ issue_close */

test("issue_close comments first, then closes", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, note(1, "Fixed in v2")), reply(200, issue({ state: "closed" })));

  const { payload, ok } = await issueClose({ repo: "g/r", number: 7, comment: "Fixed in v2" });

  assert.equal(ok, true);
  assert.equal(calls[0].url, `${BASE}/notes`);
  assert.deepEqual(calls[0].body, { body: "Fixed in v2" });
  assert.equal(calls[1].url, BASE);
  assert.equal(calls[1].method, "PUT");
  assert.deepEqual(calls[1].body, { state_event: "close" });
  assert.deepEqual(payload, {
    summary: "Closed issue #7 in g/r.",
    number: 7,
    state: "closed",
    html_url: "https://gitlab.com/g/r/-/issues/7",
  });
  expectOutput(close, payload);
});

test("issue_close without a comment makes one request, and a blank comment is none", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ state: "closed" })), reply(200, issue({ state: "closed" })));

  await issueClose({ repo: "g/r", number: 7 });
  await issueClose({ repo: "g/r", number: 7, comment: "   " });

  assert.deepEqual(calls.map((call) => call.method), ["PUT", "PUT"]);
});

test("issue_close says the comment was already posted when closing fails, and when GitLab left it open", async (t) => {
  withToken(t);
  mockApi(t, reply(201, note(1, "why")), reply(403, { message: "403 Forbidden" }), reply(200, issue({ state: "opened" })));

  await assert.rejects(
    issueClose({ repo: "g/r", number: 7, comment: "why" }),
    isRefusal(/refused to close issue #7 of g\/r \(403\).*The comment had already been posted\.$/),
  );

  const { payload } = await issueClose({ repo: "g/r", number: 7 });
  assert.equal(payload.summary, "GitLab answered, but issue #7 in g/r is still opened.");
  expectOutput(close, payload);
});

test("issue_close refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(issueClose({ repo: "g/r" }), isRefusal(/`number` is required/));
  await assert.rejects(issueClose({ repo: "g/r", number: 7, comment: 4 }), isRefusal(/`comment` must be a string/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ issue_assign */

test("issue_assign adds people to those already assigned", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, issue({ assignees: [user(11, "alice")] })),
    reply(200, [user(13, "carol")]),
    reply(200, issue({ assignees: [user(11, "alice"), user(13, "carol")] })),
  );

  const { payload, ok } = await issueAssign({ repo: "g/r", number: 7, assignees: ["@carol"] });

  assert.equal(ok, true);
  assert.equal(calls[0].url, BASE);
  assert.equal(calls[1].url, `${API}/users?username=carol`);
  assert.equal(calls[2].method, "PUT");
  assert.deepEqual(calls[2].body, { assignee_ids: [11, 13] });
  assert.deepEqual(payload, { summary: "Assignees of #7 in g/r: alice, carol.", number: 7, assignees: ["alice", "carol"] });
  expectOutput(assign, payload);
});

test("issue_assign does not write when the people are already assigned", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue()), reply(200, [user(11, "Alice")]));

  const { payload } = await issueAssign({ repo: "g/r", number: 7, assignees: ["alice"] });

  assert.deepEqual(calls.map((call) => call.method), ["GET", "GET"]);
  assert.deepEqual(payload.assignees, ["alice"]);
  assert.equal("not_applied" in payload, false);
});

test("issue_assign says which people GitLab did not apply", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, issue({ assignees: [] })),
    reply(200, [user(11, "alice")]),
    reply(200, [user(13, "carol")]),
    reply(200, issue({ assignees: [user(11, "alice")] })),
  );

  const { payload } = await issueAssign({ repo: "g/r", number: 7, assignees: ["alice", "carol"] });

  assert.deepEqual(payload.not_applied, ["carol"]);
  assert.match(payload.summary, /^Assignees of #7 in g\/r: alice\. GitLab did not assign: carol\. A free plan keeps one assignee per issue\.$/);
  expectOutput(assign, payload);
});

test("issue_assign removes people, and reports those who were not assigned", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, issue({ assignees: [user(11, "alice"), user(13, "carol")] })),
    reply(200, issue({ assignees: [user(13, "carol")] })),
  );

  const { payload } = await issueAssign({ repo: "g/r", number: 7, assignees: ["Alice", "dave"], action: "remove" });

  assert.equal(calls.length, 2, "nobody is looked up to be removed");
  assert.deepEqual(calls[1].body, { assignee_ids: [13] });
  assert.deepEqual(payload.assignees, ["carol"]);
  assert.deepEqual(payload.not_applied, ["dave"]);
  assert.match(payload.summary, /Not removed: dave\. They were not assigned, or GitLab kept them\./);
  expectOutput(assign, payload);
});

test("issue_assign unassigns everyone with the id 0", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ assignees: [user(11, "alice")] })), reply(200, issue({ assignees: [] })), reply(200, issue({ assignees: [] })));

  const removed = await issueAssign({ repo: "g/r", number: 7, assignees: ["alice"], action: "remove" });
  assert.deepEqual(calls[1].body, { assignee_ids: [0] });
  assert.equal(removed.payload.summary, "Assignees of #7 in g/r: none.");

  const cleared = await issueAssign({ repo: "g/r", number: 7, assignees: [], action: "set" });
  assert.equal(calls.length, 3, "set reads nothing and looks nobody up");
  assert.deepEqual(calls[2].body, { assignee_ids: [0] });
  assert.deepEqual(cleared.payload.assignees, []);
});

test("issue_assign replaces the assignees with set", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [user(13, "carol")]), reply(200, issue({ assignees: [user(13, "carol")] })));

  const { payload } = await issueAssign({ repo: "g/r", number: 7, assignees: ["carol"], action: "set" });

  assert.equal(calls.length, 2);
  assert.equal(calls[1].method, "PUT");
  assert.deepEqual(calls[1].body, { assignee_ids: [13] });
  assert.deepEqual(payload.assignees, ["carol"]);
});

test("issue_assign refuses an unknown user before changing anything, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue()), reply(200, []));

  await assert.rejects(issueAssign({ repo: "g/r", number: 7 }), isRefusal(/`assignees` is required/));
  await assert.rejects(issueAssign({ repo: "g/r", number: 7, assignees: [] }), isRefusal(/at least one username\. Only action set may be empty/));
  await assert.rejects(issueAssign({ repo: "g/r", number: 7, assignees: ["a"], action: "toggle" }), isRefusal(/`action` must be one of add, remove, set/));
  await assert.rejects(issueAssign({ repo: "g/r", assignees: ["a"] }), isRefusal(/`number` is required/));
  assert.equal(calls.length, 0);

  await assert.rejects(issueAssign({ repo: "g/r", number: 7, assignees: ["ghost"] }), isRefusal(/GitLab has no user named 'ghost', so nothing was changed/));
  assert.ok(calls.every((call) => call.method === "GET"));
});

/* ------------------------------------------------------------------ issue_label */

test("issue_label adds, removes and replaces labels in one request each", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, issue({ labels: ["bug", "p1", "ui"] })),
    reply(200, issue({ labels: ["bug", "ui"] })),
    reply(200, issue({ labels: ["wontfix"] })),
  );

  const added = await issueLabel({ repo: "g/r", number: 7, labels: ["ui", "p1"] });
  assert.equal(calls[0].url, BASE);
  assert.equal(calls[0].method, "PUT");
  assert.deepEqual(calls[0].body, { add_labels: "ui,p1" });
  assert.deepEqual(added.payload, { summary: "Labels on #7 in g/r: bug, p1, ui.", number: 7, labels: ["bug", "p1", "ui"] });
  expectOutput(label, added.payload);

  const removed = await issueLabel({ repo: "g/r", number: 7, labels: ["p1"], action: "remove" });
  assert.deepEqual(calls[1].body, { remove_labels: "p1" });
  assert.deepEqual(removed.payload.labels, ["bug", "ui"]);

  const replaced = await issueLabel({ repo: "g/r", number: 7, labels: ["wontfix"], action: "set" });
  assert.deepEqual(calls[2].body, { labels: "wontfix" });
  assert.deepEqual(replaced.payload.labels, ["wontfix"]);
});

test("issue_label removes every label with an empty set, and nothing else may be empty", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ labels: [] })));

  const { payload } = await issueLabel({ repo: "g/r", number: 7, labels: [], action: "set" });
  assert.deepEqual(calls[0].body, { labels: "" });
  assert.equal(payload.summary, "Labels on #7 in g/r: none.");
  expectOutput(label, payload);

  await assert.rejects(issueLabel({ repo: "g/r", number: 7, labels: [] }), isRefusal(/at least one label\. Only action set may be empty/));
  await assert.rejects(issueLabel({ repo: "g/r", number: 7, labels: [], action: "remove" }), isRefusal(/at least one label/));
});

test("issue_label says which labels are not as asked", async (t) => {
  withToken(t);
  mockApi(t, reply(200, issue({ labels: ["Bug"] })), reply(200, issue({ labels: ["bug", "p1"] })));

  const added = await issueLabel({ repo: "g/r", number: 7, labels: ["bug", "p2"] });
  assert.deepEqual(added.payload.not_applied, ["p2"]);
  assert.match(added.payload.summary, /GitLab did not apply: p2\.$/);

  const removed = await issueLabel({ repo: "g/r", number: 7, labels: ["BUG"], action: "remove" });
  assert.deepEqual(removed.payload.not_applied, ["BUG"]);
  assert.match(removed.payload.summary, /GitLab kept: BUG\.$/);
  expectOutput(label, removed.payload);
});

test("issue_label refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue()));

  await assert.rejects(issueLabel({ repo: "g/r", number: 7 }), isRefusal(/`labels` is required/));
  await assert.rejects(issueLabel({ repo: "g/r", number: 7, labels: ["a,b"] }), isRefusal(/`labels` may not contain a comma/));
  await assert.rejects(issueLabel({ repo: "g/r", number: 7, labels: ["a"], action: "x" }), isRefusal(/`action` must be one of add, remove, set/));
  await assert.rejects(issueLabel({ repo: "g/r", labels: ["a"] }), isRefusal(/`number` is required/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ issue_lock */

test("issue_lock locks by default and unlocks on request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ discussion_locked: true })), reply(200, issue({ discussion_locked: false })));

  const locked = await issueLock({ repo: "g/r", number: 7 });
  assert.equal(calls[0].url, BASE);
  assert.equal(calls[0].method, "PUT");
  assert.deepEqual(calls[0].body, { discussion_locked: true });
  assert.deepEqual(locked.payload, { summary: "Locked #7 in g/r.", number: 7, locked: true });
  expectOutput(lock, locked.payload);

  const unlocked = await issueLock({ repo: "g/r", number: 7, locked: false });
  assert.deepEqual(calls[1].body, { discussion_locked: false });
  assert.deepEqual(unlocked.payload, { summary: "Unlocked #7 in g/r.", number: 7, locked: false });
});

test("issue_lock reports the state GitLab answered with, and refuses bad arguments", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, issue({ discussion_locked: null })));

  await assert.rejects(issueLock({ repo: "g/r", number: 7, locked: "yes" }), isRefusal(/`locked` must be true or false/));
  await assert.rejects(issueLock({ repo: "g/r" }), isRefusal(/`number` is required/));
  assert.equal(calls.length, 0);

  const { payload } = await issueLock({ repo: "g/r", number: 7 });
  assert.equal(payload.summary, "GitLab answered, but #7 in g/r is still unlocked.");
  assert.equal(payload.locked, false);
  expectOutput(lock, payload);
});
