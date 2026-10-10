import assert from "node:assert/strict";
import test from "node:test";
import { MAX_FILE_CHARS, MAX_WRITE_BYTES } from "../src/config.js";
import { TOOL as create, handle as fileCreate } from "../src/tools/file-create.js";
import { TOOL as del, handle as fileDelete } from "../src/tools/file-delete.js";
import { TOOL as dir, handle as directoryList } from "../src/tools/directory-list.js";
import { TOOL as read, handle as fileRead } from "../src/tools/file-read.js";
import { TOOL as update, handle as fileUpdate } from "../src/tools/file-update.js";
import { expectOutput, isRefusal, mockApi, reply, withToken, withoutToken } from "./helpers.js";

const API = "https://gitlab.com/api/v4";
const BLOB = "f30e840422a0b9b0a0e0cc1a11118c6356801cf5";
const LAST = "3458caded2072cc2a9a037bbb4f3b9af842ff8ea";

const b64 = (text) => Buffer.from(text, "utf8").toString("base64");

/** A file as GitLab returns it. */
const file = (text, overrides = {}) => ({
  file_name: "index.js",
  file_path: "src/index.js",
  size: Buffer.byteLength(text),
  encoding: "base64",
  content: b64(text),
  ref: "HEAD",
  blob_id: BLOB,
  commit_id: "aaaa",
  last_commit_id: LAST,
  ...overrides,
});

/* ------------------------------------------------------------------ file_read */

test("file_read reads a file at HEAD, which is the default branch, with its ids", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, file("console.log(1);\n")));

  const { payload } = await fileRead({ repo: "g/r", path: "/src/index.js" });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/files/src%2Findex.js?ref=HEAD`);
  assert.deepEqual(payload, {
    summary: "Read src/index.js (16 bytes) from g/r.",
    path: "src/index.js",
    blob_id: BLOB,
    last_commit_id: LAST,
    size: 16,
    text: "console.log(1);\n",
    truncated: false,
  });
  expectOutput(read, payload);
});

test("file_read reads at a branch, a tag or a sha, and decodes UTF-8", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, file("héllo ✓")));

  const { payload } = await fileRead({ repo: "g/r", path: "a b/ü.txt", ref: "release/1.0" });

  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/files/a%20b%2F%C3%BC.txt?ref=release%2F1.0`);
  assert.equal(payload.text, "héllo ✓");
  assert.match(payload.summary, / at release\/1\.0\.$/);
});

test("file_read cuts a long file at the limit and says so", async (t) => {
  withToken(t);
  mockApi(t, reply(200, file("x".repeat(MAX_FILE_CHARS + 500))));

  const { payload } = await fileRead({ repo: "g/r", path: "big.txt" });

  assert.equal(payload.text.length, MAX_FILE_CHARS);
  assert.equal(payload.truncated, true);
  assert.match(payload.summary, /, cut at 25000 characters\.$/);
  expectOutput(read, payload);
});

test("file_read refuses a binary file, a file over 1 MB, and a body that is not a file", async (t) => {
  withToken(t);
  mockApi(
    t,
    reply(200, { ...file(""), content: Buffer.from([0x50, 0x4b, 0x00, 0x03]).toString("base64"), size: 4 }),
    reply(200, { ...file("x"), size: 2_000_000 }),
    reply(200, { message: "hello" }),
  );

  await assert.rejects(fileRead({ repo: "g/r", path: "a.zip" }), isRefusal(/`path` 'a\.zip' is a binary file \(4 bytes\)/));
  await assert.rejects(fileRead({ repo: "g/r", path: "big.bin" }), isRefusal(/is 2000000 bytes, over the 1000000 this tool reads\. Fetch it with git/));
  await assert.rejects(fileRead({ repo: "g/r", path: "x" }), isRefusal(/not a file this server recognizes/));
});

test("file_read says there is no such file, and that a directory is not a file", async (t) => {
  withToken(t);
  mockApi(t, reply(404, { message: "404 File Not Found" }));

  await assert.rejects(
    fileRead({ repo: "g/r", path: "doc", ref: "main" }),
    isRefusal(/^There is no file 'doc' in g\/r at main \(404\): 404 File Not Found\. Check the path with directory_list; a directory cannot be read as a file\./),
  );
});

test("file_read refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, file("")));

  await assert.rejects(fileRead({ repo: "g/r" }), isRefusal(/`path` is required/));
  await assert.rejects(fileRead({ repo: "g/r", path: "../x" }), isRefusal(/not a path inside the repository/));
  await assert.rejects(fileRead({ repo: "g/r", path: "a", ref: "a b" }), isRefusal(/`ref` 'a b' is not a valid/));
  await assert.rejects(fileRead({ repo: "nope", path: "a" }), isRefusal(/is not a project/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ directory_list */

test("directory_list lists the root, maps GitLab's kinds to dir, file and submodule, and pages", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(
      200,
      [
        { id: "1", name: "src", type: "tree", path: "src", mode: "040000" },
        { id: "2", name: "README.md", type: "blob", path: "README.md", mode: "100644" },
        { id: "3", name: "vendor", type: "commit", path: "vendor", mode: "160000" },
      ],
      { "x-next-page": "2" },
    ),
  );

  const { payload } = await directoryList({ repo: "g/r", per_page: 3 });

  const url = new URL(calls[0].url);
  assert.equal(url.pathname, "/api/v4/projects/g%2Fr/repository/tree");
  assert.deepEqual(Object.fromEntries(url.searchParams), { page: "1", per_page: "3" });
  assert.deepEqual(payload, {
    summary: "3 entries in the root of g/r, on page 1.",
    path: "",
    items: [
      { name: "src", type: "dir", path: "src" },
      { name: "README.md", type: "file", path: "README.md" },
      { name: "vendor", type: "submodule", path: "vendor" },
    ],
    page: 1,
    per_page: 3,
    has_more: true,
  });
  expectOutput(dir, payload);
});

test("directory_list sends the path and the ref, and says when a page is empty", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, [{ name: "a.js", type: "blob", path: "src/tools/a.js" }]), reply(200, []));

  const one = await directoryList({ repo: "g/r", path: "/src/tools/", ref: "dev", page: 1 });
  assert.deepEqual(Object.fromEntries(new URL(calls[0].url).searchParams), { path: "src/tools", ref: "dev", page: "1", per_page: "30" });
  assert.equal(one.payload.summary, "1 entry in 'src/tools' of g/r at dev, on page 1.");

  const none = await directoryList({ repo: "g/r", path: "src", page: 5 });
  assert.equal(none.payload.summary, "No entries in 'src' of g/r on page 5.");
  assert.equal(none.payload.has_more, false);
});

test("directory_list says there is no such directory, and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(404, { message: "404 invalid revision or path Not Found" }));

  await assert.rejects(directoryList({ repo: "g/r", path: "../x" }), isRefusal(/not a path inside the repository/));
  await assert.rejects(directoryList({ repo: "g/r", ref: "a b" }), isRefusal(/`ref` 'a b' is not a valid/));
  await assert.rejects(directoryList({ repo: "g/r", per_page: 101 }), isRefusal(/`per_page` must be between 1 and 100/));
  assert.equal(calls.length, 0);

  await assert.rejects(
    directoryList({ repo: "g/r", path: "nope" }),
    isRefusal(/^There is no directory 'nope' in g\/r \(404\): 404 invalid revision or path Not Found\. Check the path and the ref; a file cannot be listed\./),
  );
});

/* ------------------------------------------------------------------ file_create */

test("file_create finds the default branch, then commits the content as base64", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { default_branch: "main" }), reply(201, { file_path: "docs/a.md", branch: "main" }));

  const { payload } = await fileCreate({ repo: "g/r", path: "docs/a.md", content: "héllo\r\n", message: " add a " });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, `${API}/projects/g%2Fr`);
  assert.equal(calls[1].method, "POST");
  assert.equal(calls[1].url, `${API}/projects/g%2Fr/repository/files/docs%2Fa.md`);
  assert.deepEqual(calls[1].body, { branch: "main", content: b64("héllo\r\n"), encoding: "base64", commit_message: "add a" });
  assert.deepEqual(payload, { summary: "Created docs/a.md in g/r on main.", path: "docs/a.md", branch: "main" });
  expectOutput(create, payload);
});

test("file_create commits to the branch it is given with no lookup, and allows an empty file", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(201, { file_path: ".keep", branch: "feat/x" }));

  const { payload } = await fileCreate({ repo: "g/r", path: ".keep", content: "", message: "keep", branch: "feat/x" });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].body.branch, "feat/x");
  assert.equal(calls[0].body.content, "");
  assert.equal(payload.branch, "feat/x");
});

test("file_create says when the file exists, and carries GitLab's reason for any other refusal", async (t) => {
  withToken(t);
  mockApi(t, reply(400, { message: "A file with this name already exists" }), reply(400, { message: "You are not allowed to push into this branch" }));

  await assert.rejects(
    fileCreate({ repo: "g/r", path: "a", content: "x", message: "m", branch: "main" }),
    isRefusal(/^A file 'a' already exists on main in g\/r: A file with this name already exists\. Use file_update to change it\.$/),
  );
  await assert.rejects(
    fileCreate({ repo: "g/r", path: "a", content: "x", message: "m", branch: "main" }),
    isRefusal(/^GitLab could not create 'a' in g\/r on main \(400\): You are not allowed to push into this branch\.$/),
  );
});

test("file_create refuses bad arguments, content that is too large, and a project with no default branch, before it writes", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { default_branch: null }));

  await assert.rejects(fileCreate({ repo: "g/r", content: "x", message: "m" }), isRefusal(/`path` is required/));
  await assert.rejects(fileCreate({ repo: "g/r", path: "a", message: "m" }), isRefusal(/`content` is required/));
  await assert.rejects(fileCreate({ repo: "g/r", path: "a", content: "x" }), isRefusal(/`message` is required/));
  await assert.rejects(fileCreate({ repo: "g/r", path: "a", content: 5, message: "m" }), isRefusal(/`content` must be a string/));
  await assert.rejects(
    fileCreate({ repo: "g/r", path: "a", content: "x".repeat(MAX_WRITE_BYTES + 1), message: "m" }),
    isRefusal(/over the 1000000-byte limit for one write/),
  );
  assert.equal(calls.length, 0);

  await assert.rejects(fileCreate({ repo: "g/r", path: "a", content: "x", message: "m" }), isRefusal(/has no default branch yet/));
});

/* ------------------------------------------------------------------ file_update */

test("file_update commits the whole new content, and passes the last commit it is given", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { file_path: "a.txt", branch: "dev" }));

  const { payload } = await fileUpdate({ repo: "g/r", path: "a.txt", content: "two", message: "update", branch: "dev", last_commit_id: LAST });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].method, "PUT");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/files/a.txt`);
  assert.deepEqual(calls[0].body, { branch: "dev", content: b64("two"), encoding: "base64", commit_message: "update", last_commit_id: LAST });
  assert.deepEqual(payload, { summary: "Updated a.txt in g/r on dev.", path: "a.txt", branch: "dev" });
  expectOutput(update, payload);
});

test("file_update sends no last_commit_id by default, finds the default branch, and explains a missing or stale file", async (t) => {
  withToken(t);
  const { calls } = mockApi(
    t,
    reply(200, { default_branch: "main" }),
    reply(200, { file_path: "a", branch: "main" }),
    reply(400, { message: "A file with this name doesn't exist" }),
    reply(400, { message: "You are attempting to update or delete a file that has changed since you started editing it" }),
  );

  await fileUpdate({ repo: "g/r", path: "a", content: "x", message: "m" });
  assert.equal("last_commit_id" in calls[1].body, false);

  await assert.rejects(
    fileUpdate({ repo: "g/r", path: "a", content: "x", message: "m", branch: "main" }),
    isRefusal(/^There is no file 'a' on main in g\/r: A file with this name doesn't exist\. Use file_create to make it/),
  );
  await assert.rejects(
    fileUpdate({ repo: "g/r", path: "a", content: "x", message: "m", branch: "main", last_commit_id: LAST }),
    isRefusal(/changed since it was read.*Read it again with file_read and use its new last_commit_id\.$/),
  );
});

test("file_update refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(fileUpdate({ repo: "g/r", path: "a", content: "x" }), isRefusal(/`message` is required/));
  await assert.rejects(fileUpdate({ repo: "g/r", path: "a", content: "x", message: "m", last_commit_id: 5 }), isRefusal(/`last_commit_id` must be a string/));
  await assert.rejects(fileUpdate({ repo: "g/r", path: "a", content: "x", message: "m", branch: "a b" }), isRefusal(/`branch` 'a b' is not a valid/));
  assert.equal(calls.length, 0);
});

/* ------------------------------------------------------------------ file_delete */

test("file_delete commits the removal with a JSON body, and says what it removed", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, null));

  const { payload } = await fileDelete({ repo: "g/r", path: "old.txt", message: "drop it", branch: "main", last_commit_id: LAST });

  assert.equal(calls[0].method, "DELETE");
  assert.equal(calls[0].url, `${API}/projects/g%2Fr/repository/files/old.txt`);
  assert.deepEqual(calls[0].body, { branch: "main", commit_message: "drop it", last_commit_id: LAST });
  assert.deepEqual(payload, { summary: "Deleted old.txt from g/r on main.", deleted: true, path: "old.txt", branch: "main" });
  expectOutput(del, payload);
});

test("file_delete finds the default branch, explains a missing file, and refuses bad arguments before any request", async (t) => {
  withToken(t);
  const { calls } = mockApi(t, reply(200, { default_branch: "main" }), reply(204), reply(400, { message: "A file with this name doesn't exist" }));

  await fileDelete({ repo: "g/r", path: "a", message: "m" });
  assert.equal(calls[1].body.branch, "main");
  assert.equal("last_commit_id" in calls[1].body, false);
  await assert.rejects(fileDelete({ repo: "g/r", path: "a", message: "m", branch: "main" }), isRefusal(/^There is no file 'a' on main in g\/r/));

  const before = calls.length;
  await assert.rejects(fileDelete({ repo: "g/r", path: "a" }), isRefusal(/`message` is required/));
  await assert.rejects(fileDelete({ repo: "g/r", message: "m" }), isRefusal(/`path` is required/));
  assert.equal(calls.length, before);
});

test("the write tools are held to the no-token rule only after their arguments check out", async (t) => {
  withoutToken(t);
  const { calls } = mockApi(t, reply(200, {}));

  await assert.rejects(fileCreate({ repo: "g/r", path: "a", content: "x", message: "m", branch: "main" }), isRefusal(/^MCP_AUTH_TOKEN is not set\./));
  assert.equal(calls.length, 0);
});
