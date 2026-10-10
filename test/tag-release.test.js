import assert from "node:assert/strict";
import test from "node:test";
import { TOOL as check, handle as tagCheck } from "../src/tools/tag-check.js";
import { TOOL as tagDel, handle as tagDelete } from "../src/tools/tag-delete.js";
import { TOOL as getAll, handle as tagGetAll } from "../src/tools/tag-get-all.js";
import { TOOL as get, handle as tagGet } from "../src/tools/tag-get.js";
import { TOOL as create, handle as releaseCreate } from "../src/tools/release-create.js";
import { TOOL as relDel, handle as releaseDelete } from "../src/tools/release-delete.js";
import { TOOL as relList, handle as releaseList } from "../src/tools/release-list.js";
import { TOOL as read, handle as releaseRead } from "../src/tools/release-read.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const API = "https://gitlab.com/api/v4";
const COMMIT = "0123456789abcdef0123456789abcdef01234567";
const TAG_OBJECT = "fedcba9876543210fedcba9876543210fedcba98";

/** A lightweight tag as GitLab returns it. */
const light = (overrides = {}) => ({
  name: "v1.0.0",
  message: null,
  target: COMMIT,
  commit: { id: COMMIT, short_id: COMMIT.slice(0, 8), title: "Release" },
  release: null,
  protected: false,
  created_at: null,
  ...overrides,
});

/** An annotated tag: its target is the tag object, not the commit. */
const annotated = (overrides = {}) =>
  light({ message: "Version 1.0.0", target: TAG_OBJECT, created_at: "2026-10-01T10:00:00.000+00:00", ...overrides });

/** A release as GitLab returns it. */
const release = (overrides = {}) => ({
  name: "Version 1.0.0",
  tag_name: "v1.0.0",
  description: "Notes\n\n- one",
  created_at: "2026-10-01T10:00:00.000Z",
  released_at: "2026-10-01T10:00:00.000Z",
  upcoming_release: false,
  assets: {
    count: 3,
    sources: [{ format: "zip", url: "https://gitlab.com/g/r/-/archive/v1.0.0/r-v1.0.0.zip" }],
    links: [{ id: 1, name: "binary", url: "https://example.com/b", direct_asset_url: "https://gitlab.com/g/r/-/releases/v1.0.0/downloads/binary", link_type: "other" }],
  },
  _links: { self: "https://gitlab.com/g/r/-/releases/v1.0.0" },
  ...overrides,
});

const FACTS = {
  tag: "v1.0.0",
  name: "Version 1.0.0",
  upcoming: false,
  html_url: "https://gitlab.com/g/r/-/releases/v1.0.0",
  released_at: "2026-10-01T10:00:00.000Z",
  body: "Notes\n\n- one",
  assets: [
    { name: "binary", url: "https://gitlab.com/g/r/-/releases/v1.0.0/downloads/binary" },
    { name: "source.zip", url: "https://gitlab.com/g/r/-/archive/v1.0.0/r-v1.0.0.zip" },
  ],
};

/* ------------------------------------------------------------------ tag_check */

test("tag_check says a tag exists, and whether it is annotated", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, light()), reply(200, annotated({ name: "rel/2" })));

  const lightweight = await tagCheck({ repo: "g/r", tag: "v1.0.0" });
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/tags/v1.0.0`);
  assert.deepEqual(lightweight.payload, {
    summary: "Tag v1.0.0 exists in g/r (lightweight) and points at 0123456.",
    exists: true,
    tag: "v1.0.0",
    sha: COMMIT,
    annotated: false,
  });
  expectOutput(check, lightweight.payload);

  const tagged = await tagCheck({ repo: "g/r", tag: "rel/2" });
  assert.equal(calls[1].url, `${API}/projects/g%2Fr/repository/tags/rel%2F2`);
  assert.equal(tagged.payload.annotated, true);
  assert.equal(tagged.payload.sha, COMMIT, "the commit, not the tag object");
});

test("tag_check treats a missing tag as an answer, but a missing project as a refusal", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "404 Tag Not Found" }), reply(404, { message: "404 Project Not Found" }));

  const { payload, ok } = await tagCheck({ repo: "g/r", tag: "v9" });
  assert.equal(ok, true);
  assert.deepEqual(payload, { summary: "Tag v9 does not exist in g/r.", exists: false, tag: "v9" });
  expectOutput(check, payload);

  await assert.rejects(tagCheck({ repo: "g/gone", tag: "v9" }), isRefusal(/could not find the project g\/gone \(404\): 404 Project Not Found\. Check the spelling/));
});

test("tag_check refuses a body that is not a tag, and bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { nope: true }));

  await assert.rejects(tagCheck({ repo: "g/r" }), isRefusal(/`tag` is required/));
  await assert.rejects(tagCheck({ repo: "g/r", tag: "a b" }), isRefusal(/`tag` 'a b' is not a valid/));
  assert.equal(calls.length, 0);

  await assert.rejects(tagCheck({ repo: "g/r", tag: "v1" }), isRefusal(/not a tag this server recognizes/));
});

/* ------------------------------------------------------------------ tag_get */

test("tag_get reads a lightweight tag and an annotated one", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, light({ release: { tag_name: "v1.0.0", description: "x" } })), reply(200, annotated({ protected: true })));

  const lightweight = await tagGet({ repo: "g/r", tag: "v1.0.0" });
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/tags/v1.0.0`);
  assert.deepEqual(lightweight.payload, {
    summary: "Tag v1.0.0 is a lightweight tag and points at 0123456.",
    tag: "v1.0.0",
    sha: COMMIT,
    target: COMMIT,
    annotated: false,
    protected: false,
    has_release: true,
  });
  expectOutput(get, lightweight.payload);

  const tagged = await tagGet({ repo: "g/r", tag: "v1.0.0" });
  assert.equal(tagged.payload.summary, "Tag v1.0.0 is annotated and points at 0123456.");
  assert.deepEqual(
    [tagged.payload.target, tagged.payload.message, tagged.payload.date, tagged.payload.protected, tagged.payload.has_release],
    [TAG_OBJECT, "Version 1.0.0", "2026-10-01T10:00:00.000+00:00", true, false],
  );
  expectOutput(get, tagged.payload);
});

test("tag_get says there is no such tag, and refuses a body that is not one", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "404 Tag Not Found" }), reply(200, {}));

  await assert.rejects(tagGet({ repo: "g/r", tag: "v9" }), isRefusal(/^There is no tag 'v9' in g\/r\. List them with tag_get_all\.$/));
  await assert.rejects(tagGet({ repo: "g/r", tag: "v1" }), isRefusal(/not a tag this server recognizes/));
});

/* ------------------------------------------------------------------ tag_get_all */

test("tag_get_all lists tags with the commit each points at, searches and pages", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [light({ name: "v2" }), light({ name: "v1", commit: { id: TAG_OBJECT } })], { "x-next-page": "2" }), reply(200, []));

  const { payload } = await tagGetAll({ repo: "g/r", search: " ^v ", per_page: 2 });

  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/api/v4/projects/g%2Fr/repository/tags");
  assert.deepEqual(Object.fromEntries(url.searchParams), { search: "^v", page: "1", per_page: "2" });
  assert.deepEqual(payload, {
    summary: "2 tags matching '^v' of g/r on page 1.",
    items: [
      { name: "v2", sha: COMMIT },
      { name: "v1", sha: TAG_OBJECT },
    ],
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(getAll, payload);

  const none = await tagGetAll({ repo: "g/r", page: 4 });
  assert.equal(none.payload.summary, "g/r has no tags on page 4.");
});

/* ------------------------------------------------------------------ tag_delete */

test("tag_delete removes the tag and says it left releases alone", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(204), reply(403, { message: "403 Forbidden" }));

  const { payload } = await tagDelete({ repo: "g/r", tag: "rel/2" });
  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/tags/rel%2F2`);
  assert.deepEqual(payload, { summary: "Deleted tag rel/2 from g/r.", deleted: true, tag: "rel/2" });
  expectOutput(tagDel, payload);
  assert.match(tagDel.description, /does not delete a release/);

  await assert.rejects(
    tagDelete({ repo: "g/r", tag: "v1" }),
    isRefusal(/^GitLab would not delete the tag 'v1' from g\/r \(403\): 403 Forbidden\. It may not exist.*protected/),
  );
});

/* ------------------------------------------------------------------ release_create */

test("release_create makes a release, naming the default branch for a tag that might not exist", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { default_branch: "main" }), reply(201, release()));

  const { payload } = await releaseCreate({ repo: "g/r", tag: "v1.0.0", name: " Version 1.0.0 ", body: "Notes\n\n- one" });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, `${API}/projects/g%2Fr`);
  assert.equal(calls[1].method, "POST");
  assert.equal(calls[1].url, `${API}/projects/g%2Fr/releases`);
  assert.deepEqual(calls[1].body, { tag_name: "v1.0.0", ref: "main", name: "Version 1.0.0", description: "Notes\n\n- one" });
  assert.deepEqual(payload, { summary: "Created release Version 1.0.0 (tag v1.0.0) in g/r.", ...FACTS });
  expectOutput(create, payload);
});

test("release_create takes a target and a release date, and an upcoming release says so", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, release({ upcoming_release: true, released_at: "2027-01-01T00:00:00.000Z", assets: undefined })));

  const { payload } = await releaseCreate({ repo: "g/r", tag: "v2", target: "release/2", released_at: "2027-01-01" });

  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].body, { tag_name: "v2", ref: "release/2", released_at: "2027-01-01" });
  assert.equal(payload.upcoming, true);
  assert.match(payload.summary, /^Created upcoming release Version 1\.0\.0/);
  assert.deepEqual(payload.assets, []);
  expectOutput(create, payload);
});

test("release_create says when the tag already has a release, and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(409, { message: "Release already exists" }));

  await assert.rejects(releaseCreate({ repo: "g/r" }), isRefusal(/`tag` is required/));
  await assert.rejects(releaseCreate({ repo: "g/r", tag: "a b" }), isRefusal(/`tag` 'a b' is not a valid/));
  await assert.rejects(releaseCreate({ repo: "g/r", tag: "v1", target: "a..b" }), isRefusal(/`target` 'a..b' is not a valid/));
  await assert.rejects(releaseCreate({ repo: "g/r", tag: "v1", released_at: "soon" }), isRefusal(/`released_at` must be an ISO 8601 date/));
  await assert.rejects(releaseCreate({ repo: "g/r", tag: "v1", body: 3 }), isRefusal(/`body` must be a string/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    releaseCreate({ repo: "g/r", tag: "v1", target: "main" }),
    isRefusal(/could not create a release for 'v1' in g\/r \(409\): Release already exists\. A release for that tag already exists; read it with release_read/),
  );
});

/* ------------------------------------------------------------------ release_read */

test("release_read reads the release of a tag, and the latest through its permanent address", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, release()), reply(200, release({ tag_name: "v1.1.0", name: null })));

  const byTag = await releaseRead({ repo: "g/r", tag: "rel/1" });
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/releases/rel%2F1`);
  assert.deepEqual(byTag.payload, { summary: "g/r has release Version 1.0.0 (tag v1.0.0).", ...FACTS });
  expectOutput(read, byTag.payload);

  const latest = await releaseRead({ repo: "g/r" });
  assert.equal(calls[1].url, `${API}/projects/g%2Fr/releases/permalink/latest`);
  assert.equal(latest.payload.summary, "g/r has release v1.1.0 (tag v1.1.0).");
  assert.equal("name" in latest.payload, false);
});

test("release_read says there is no such release, or none yet", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "404 Not found" }), reply(404, { message: "404 Not found" }), reply(200, { nope: true }));

  await assert.rejects(releaseRead({ repo: "g/r", tag: "v9" }), isRefusal(/^g\/r has no release for the tag 'v9'\. List what exists with release_list\.$/));
  await assert.rejects(releaseRead({ repo: "g/r" }), isRefusal(/^g\/r has no release yet\./));
  await assert.rejects(releaseRead({ repo: "g/r" }), isRefusal(/not a release this server recognizes/));
});

/* ------------------------------------------------------------------ release_list */

test("release_list asks newest first, lists each release, and pages", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, [release({ upcoming_release: true }), release({ tag_name: "v0.9.0", name: null, released_at: undefined })], { "x-next-page": "2" }),
    reply(200, []),
  );

  const { payload } = await releaseList({ repo: "g/r", per_page: 2 });

  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/api/v4/projects/g%2Fr/releases");
  assert.deepEqual(Object.fromEntries(url.searchParams), { order_by: "released_at", sort: "desc", page: "1", per_page: "2" });
  assert.deepEqual(payload, {
    summary: "2 releases of g/r on page 1.",
    items: [
      { tag: "v1.0.0", name: "Version 1.0.0", upcoming: true, released_at: "2026-10-01T10:00:00.000Z" },
      { tag: "v0.9.0", upcoming: false },
    ],
    page: 1,
    per_page: 2,
    has_more: true,
  });
  expectOutput(relList, payload);

  assert.equal((await releaseList({ repo: "g/r", page: 3 })).payload.summary, "g/r has no releases on page 3.");
});

/* ------------------------------------------------------------------ release_delete */

test("release_delete removes the release and keeps the tag", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, release()), reply(404, { message: "404 Not found" }));

  const { payload } = await releaseDelete({ repo: "g/r", tag: "v1.0.0" });

  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/releases/v1.0.0`);
  assert.deepEqual(payload, { summary: "Deleted the release for v1.0.0 of g/r. Its tag was kept.", deleted: true, tag: "v1.0.0" });
  expectOutput(relDel, payload);

  await assert.rejects(releaseDelete({ repo: "g/r", tag: "v9" }), isRefusal(/^g\/r has no release for the tag 'v9'\./));
});

test("release_delete refuses bad arguments before any request", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(releaseDelete({ repo: "g/r" }), isRefusal(/`tag` is required/));
  await assert.rejects(releaseDelete({ repo: "g/r", tag: "a b" }), isRefusal(/`tag` 'a b' is not a valid/));
  assert.equal(calls.length, 0);
});
