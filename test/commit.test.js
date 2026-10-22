import assert from "node:assert/strict";
import test from "node:test";
import { MAX_COMMITS_SHOWN, MAX_DIFF_CHARS, MAX_FILES_SHOWN } from "../src/config/config.js";
import { countLines, fileStatus, summarizeFile, unifiedDiff } from "../src/utils/diffs.js";
import { TOOL as compare, handle as commitCompare } from "../src/controllers/commit-compare.js";
import { TOOL as list, handle as commitList } from "../src/controllers/commit-list.js";
import { TOOL as read, handle as commitRead } from "../src/controllers/commit-read.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const API = "https://gitlab.com/api/v4";
const SHA = "0123456789abcdef0123456789abcdef01234567";
const PARENT = "fedcba9876543210fedcba9876543210fedcba98";

/** A commit as GitLab returns it. */
const commit = (overrides = {}) => ({
  id: SHA,
  short_id: SHA.slice(0, 8),
  title: "Add login",
  message: "Add login\n\nWith a form.\n",
  author_name: "Ada",
  author_email: "ada@example.com",
  authored_date: "2026-10-01T12:00:00.000+00:00",
  committed_date: "2026-10-01T12:00:00.000+00:00",
  created_at: "2026-10-01T12:00:00.000+00:00",
  parent_ids: [PARENT],
  web_url: `https://gitlab.com/g/r/-/commit/${SHA}`,
  stats: { additions: 4, deletions: 1, total: 5 },
  ...overrides,
});

/** A changed file as GitLab returns it in a diff. */
const changed = (overrides = {}) => ({
  old_path: "src/a.js",
  new_path: "src/a.js",
  new_file: false,
  renamed_file: false,
  deleted_file: false,
  diff: "@@ -1,2 +1,3 @@\n keep\n-old\n+new\n+more\n",
  ...overrides,
});

/* ------------------------------------------------------------------ diffs */

test("diffs: a file's status, its line counts, and the unified text with the headers GitLab leaves out", () => {
  assert.equal(fileStatus({ new_file: true }), "added");
  assert.equal(fileStatus({ deleted_file: true }), "removed");
  assert.equal(fileStatus({ renamed_file: true }), "renamed");
  assert.equal(fileStatus({}), "modified");

  assert.deepEqual(countLines("@@ -1 +1 @@\n-a\n-b\n+c\n \\ No newline at end of file\n"), { additions: 1, deletions: 2 });
  assert.deepEqual(countLines("@@ -1 +1 @@\n--- a\n+++ b\n"), { additions: 1, deletions: 1 }, "a line that begins with dashes is still a line");
  assert.deepEqual(countLines(undefined), { additions: 0, deletions: 0 });

  assert.deepEqual(summarizeFile(changed()), { filename: "src/a.js", status: "modified", additions: 2, deletions: 1 });

  assert.equal(
    unifiedDiff([changed({ old_path: "a", new_path: "b", renamed_file: true, diff: "@@ -1 +1 @@\n-x\n+y\n" })]),
    "diff --git a/a b/b\n--- a/a\n+++ b/b\n@@ -1 +1 @@\n-x\n+y\n",
  );
  assert.equal(
    unifiedDiff([changed({ old_path: "n", new_path: "n", new_file: true, diff: "@@ -0,0 +1 @@\n+z\n" })]),
    "diff --git a/n b/n\n--- /dev/null\n+++ b/n\n@@ -0,0 +1 @@\n+z\n",
  );
  assert.match(unifiedDiff([changed({ deleted_file: true, diff: "" , too_large: true })]), /\+\+\+ \/dev\/null\n\(GitLab left this file's diff out because it is too large\)\n$/);
});

/* ------------------------------------------------------------------ commit_read */

test("commit_read reads a commit and the files it changed, from two requests", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, commit()), reply(200, [changed(), changed({ old_path: "n", new_path: "n", new_file: true, diff: "@@ -0,0 +1 @@\n+z\n" })], { "x-next-page": "" }));

  const { payload } = await commitRead({ repo: "g/r", ref: "feat/x" });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/commits/feat%2Fx`);
  const diffUrl = new URL(calls[1].url);
  assert.equal(diffUrl.pathname, "/api/v4/projects/g%2Fr/repository/commits/feat%2Fx/diff");
  assert.deepEqual(Object.fromEntries(diffUrl.searchParams), { per_page: "100" });
  assert.deepEqual(payload, {
    summary: "Commit 0123456 by Ada: Add login",
    sha: SHA,
    message: "Add login\n\nWith a form.\n",
    author: "Ada",
    date: "2026-10-01T12:00:00.000+00:00",
    parents: [PARENT],
    additions: 4,
    deletions: 1,
    file_count: 2,
    files: [
      { filename: "src/a.js", status: "modified", additions: 2, deletions: 1 },
      { filename: "n", status: "added", additions: 1, deletions: 0 },
    ],
    truncated: false,
    html_url: `https://gitlab.com/g/r/-/commit/${SHA}`,
  });
  expectOutput(read, payload);
});

test("commit_read defaults to HEAD, and adds the diff when asked, cut at the limit", async (t) => {
  withToken(t);
  const big = changed({ diff: `@@ -1 +1 @@\n${"+x\n".repeat(MAX_DIFF_CHARS)}` });
  const { calls } = mockApi(t, reply(200, commit()), reply(200, [changed()]), reply(200, commit()), reply(200, [big]));

  const small = await commitRead({ repo: "g/r", diff: true });
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/commits/HEAD`);
  assert.equal(small.payload.diff, "diff --git a/src/a.js b/src/a.js\n--- a/src/a.js\n+++ b/src/a.js\n@@ -1,2 +1,3 @@\n keep\n-old\n+new\n+more\n");
  assert.equal(small.payload.truncated, false);
  expectOutput(read, small.payload);

  const cut = await commitRead({ repo: "g/r", diff: true });
  assert.equal(cut.payload.truncated, true);
  assert.match(cut.payload.diff, /\.\.\. \(truncated\)$/);
  expectOutput(read, cut.payload);
});

test("commit_read says when there are more files than it lists, and totals from the files when GitLab sends no stats", async (t) => {
  withToken(t);
  mockApi(t, reply(200, commit({ stats: undefined })), reply(200, [changed()], { "x-next-page": "2" }));

  const { payload } = await commitRead({ repo: "g/r" });

  assert.equal(payload.truncated, true);
  assert.deepEqual([payload.additions, payload.deletions], [2, 1]);
  assert.equal(MAX_FILES_SHOWN, 100);
});

test("commit_read refuses a missing commit, a body that is not one, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "404 Commit Not Found" }), reply(200, { nope: true }));

  await assert.rejects(commitRead({ repo: "g/r", ref: "a b" }), isRefusal(/`ref` 'a b' is not a valid/));
  await assert.rejects(commitRead({ repo: "g/r", diff: "yes" }), isRefusal(/`diff` must be true or false/));
  assert.equal(calls.length, 0);

  await assert.rejects(commitRead({ repo: "g/r", ref: "nope" }), isRefusal(/could not find what it needs to read the commit 'nope' of g\/r \(404\): 404 Commit Not Found/));
  await assert.rejects(commitRead({ repo: "g/r" }), isRefusal(/not a commit this server recognizes/));
});

/* ------------------------------------------------------------------ commit_list */

test("commit_list sends the filters and the paging, and lists each commit", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, [commit(), commit({ id: PARENT, title: "Start", author_name: undefined, authored_date: undefined })], { "x-next-page": "3" }),
  );

  const { payload } = await commitList({
    repo: "g/r",
    ref: "release/1.0",
    path: "/src/",
    author: " Ada ",
    since: "2026-10-01",
    until: "2026-10-09T10:00:00Z",
    page: 2,
    per_page: 2,
  });

  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/api/v4/projects/g%2Fr/repository/commits");
  assert.deepEqual(Object.fromEntries(url.searchParams), {
    ref_name: "release/1.0",
    path: "src",
    author: "Ada",
    since: "2026-10-01",
    until: "2026-10-09T10:00:00Z",
    page: "2",
    per_page: "2",
  });
  assert.deepEqual(payload, {
    summary: "2 commits on release/1.0 of g/r on page 2.",
    items: [
      { sha: SHA, message: "Add login", author: "Ada", date: "2026-10-01T12:00:00.000+00:00" },
      { sha: PARENT, message: "Start" },
    ],
    page: 2,
    per_page: 2,
    has_more: true,
  });
  expectOutput(list, payload);
});

test("commit_list sends no filters by default, reads the link header when GitLab sends no next page, and reports an empty page", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [commit()], { link: '<https://gitlab.com/x?page=2>; rel="next"' }), reply(200, []));

  const some = await commitList({ repo: "g/r" });
  assert.deepEqual(Object.fromEntries(new URL(calls[0].url).searchParams), { page: "1", per_page: "30" });
  assert.equal(some.payload.summary, "1 commit of g/r on page 1.");
  assert.equal(some.payload.has_more, true);

  const none = await commitList({ repo: "g/r", ref: "dev", page: 9 });
  assert.equal(none.payload.summary, "No commits on dev of g/r match on page 9.");
});

test("commit_list refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, []));

  await assert.rejects(commitList({ repo: "g/r", since: "yesterday" }), isRefusal(/`since` must be an ISO 8601 date/));
  await assert.rejects(commitList({ repo: "g/r", until: "2026-13-45" }), isRefusal(/`until` must be an ISO 8601 date/));
  await assert.rejects(commitList({ repo: "g/r", path: "../x" }), isRefusal(/not a path inside the repository/));
  await assert.rejects(commitList({ repo: "g/r", ref: "a..b" }), isRefusal(/`ref` 'a..b' is not a valid/));
  await assert.rejects(commitList({ repo: "g/r", author: 4 }), isRefusal(/`author` must be a string/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ commit_compare */

test("commit_compare asks both ways to say how far ahead and behind, and lists commits and files", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, {
      commit: commit(),
      commits: [commit({ id: SHA, created_at: "2026-10-02T00:00:00Z" }), commit({ id: PARENT, title: "Start", created_at: "2026-10-01T00:00:00Z" })],
      diffs: [changed()],
      compare_timeout: false,
      compare_same_ref: false,
      web_url: "https://gitlab.com/g/r/-/compare/main...feat",
    }),
    reply(200, { commit: null, commits: [{ id: "c" }], diffs: [] }),
  );

  const { payload } = await commitCompare({ repo: "g/r", base: "main", head: "feat/x" });

  assert.equal(calls.length, 2);
  assert.deepEqual(Object.fromEntries(new URL(calls[0].url).searchParams), { from: "main", to: "feat/x" });
  assert.deepEqual(Object.fromEntries(new URL(calls[1].url).searchParams), { from: "feat/x", to: "main" });
  assert.deepEqual(payload, {
    summary: "feat/x has diverged from main: 2 ahead, 1 behind.",
    status: "diverged",
    ahead_by: 2,
    behind_by: 1,
    total_commits: 2,
    commits: [
      { sha: PARENT, message: "Start", author: "Ada" },
      { sha: SHA, message: "Add login", author: "Ada" },
    ],
    file_count: 1,
    files: [{ filename: "src/a.js", status: "modified", additions: 2, deletions: 1 }],
    truncated: false,
    html_url: "https://gitlab.com/g/r/-/compare/main...feat",
  });
  expectOutput(compare, payload);
});

test("commit_compare says identical, ahead and behind in a sentence", async (t) => {
  withToken(t);
  const none = { commits: [], diffs: [] };
  const one = { commits: [commit()], diffs: [] };
  mockApi(t, reply(200, none), reply(200, none), reply(200, one), reply(200, none), reply(200, none), reply(200, one));

  assert.equal((await commitCompare({ repo: "g/r", base: "a", head: "b" })).payload.summary, "b and a are identical.");
  assert.equal((await commitCompare({ repo: "g/r", base: "a", head: "b" })).payload.summary, "b is 1 commit ahead of a.");
  assert.equal((await commitCompare({ repo: "g/r", base: "a", head: "b" })).payload.summary, "b is 1 commit behind a.");
});

test("commit_compare keeps the newest commits and the first files, and says it did", async (t) => {
  withToken(t);
  const many = Array.from({ length: MAX_COMMITS_SHOWN + 5 }, (_, i) => commit({ id: `c${i}`, title: `commit ${i}`, created_at: new Date(Date.UTC(2026, 9, 1, 0, i)).toISOString() }));
  const files = Array.from({ length: MAX_FILES_SHOWN + 3 }, (_, i) => changed({ old_path: `f${i}`, new_path: `f${i}` }));
  mockApi(t, reply(200, { commits: [...many].reverse(), diffs: files, compare_timeout: false }), reply(200, { commits: [], diffs: [] }));

  const { payload } = await commitCompare({ repo: "g/r", base: "a", head: "b" });

  assert.equal(payload.total_commits, MAX_COMMITS_SHOWN + 5);
  assert.equal(payload.commits.length, MAX_COMMITS_SHOWN);
  assert.equal(payload.commits.at(-1).message, `commit ${MAX_COMMITS_SHOWN + 4}`);
  assert.equal(payload.commits[0].message, "commit 5");
  assert.equal(payload.file_count, MAX_FILES_SHOWN + 3);
  assert.equal(payload.files.length, MAX_FILES_SHOWN);
  assert.equal(payload.truncated, true);
  expectOutput(compare, payload);
});

test("commit_compare marks a comparison GitLab timed out on as incomplete", async (t) => {
  withToken(t);
  mockApi(t, reply(200, { commits: [commit()], diffs: [], compare_timeout: true }), reply(200, { commits: [], diffs: [] }));

  assert.equal((await commitCompare({ repo: "g/r", base: "a", head: "b" })).payload.truncated, true);
});

test("commit_compare explains a missing ref, refuses a body that is not a comparison, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "404 invalid revision Not Found" }), reply(200, { nope: true }));

  await assert.rejects(commitCompare({ repo: "g/r", head: "b" }), isRefusal(/`base` is required/));
  await assert.rejects(commitCompare({ repo: "g/r", base: "a" }), isRefusal(/`head` is required/));
  await assert.rejects(commitCompare({ repo: "g/r", base: "a b", head: "b" }), isRefusal(/`base` 'a b' is not a valid/));
  assert.equal(calls.length, 0);

  await assert.rejects(commitCompare({ repo: "g/r", base: "a", head: "ghost" }), isRefusal(/could not find 'a' or 'ghost' in g\/r \(404\).*Check both names/));
  await assert.rejects(commitCompare({ repo: "g/r", base: "a", head: "b" }), isRefusal(/not a comparison this server recognizes/));
});
