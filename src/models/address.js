/**
 * Reading the arguments that end up in a GitLab URL: a repository, a reference, a path in a
 * repository. Each is validated here and returned with the form it takes in a URL, so
 * `src/services/api.js` can build a path from nothing that has not passed through this file.
 */
import { Refused } from "../utils/errors.js";
import { rawText, requiredText } from "./args.js";

// GitLab's own rule for a path segment, of a group or of a project: letters, digits, '_', '-'
// and '.', not starting with '-' or '.'.
export const SEGMENT = /^[A-Za-z0-9_][A-Za-z0-9._-]{0,254}$/;

// A project sits in a group, which may sit in up to twenty subgroups.
const MAX_GROUP_DEPTH = 20;

const MAX_PATH_DEPTH = MAX_GROUP_DEPTH + 1;

/**
 * Is this the path of a group or subgroup - or of a user's own namespace, which is one segment?
 *
 * @param {string} text
 * @returns {boolean}
 */
export function isNamespacePath(text) {
  const segments = text.split("/");
  return segments.length <= MAX_GROUP_DEPTH && segments.every((segment) => SEGMENT.test(segment));
}

/**
 * Read a project argument: its full path, or its numeric id.
 *
 * @param {unknown} value
 * @param {string} [field]
 * @returns {{ full: string, path: string }} `path` is the API prefix, `/projects/<id>`, with a path
 *   written as the single URL segment GitLab expects, slashes encoded.
 */
export function parseRepo(value, field = "repo") {
  const text = requiredText(value, field);
  if (/^[1-9]\d{0,15}$/.test(text)) return { full: text, path: `/projects/${text}` };

  const parts = text.split("/");
  if (parts.length < 2 || parts.length > MAX_PATH_DEPTH || !parts.every((part) => SEGMENT.test(part))) {
    throw new Refused(
      `\`${field}\` '${text}' is not a project. Write its full path, such as 'my-group/my-project' or ` +
        "'my-group/sub-group/my-project', or give its numeric id.",
    );
  }
  return { full: text, path: `/projects/${encodeURIComponent(text)}` };
}

// Characters git forbids in a reference name, plus whitespace and control characters.
const BAD_REF = /[\u0000- \u007f~^:?*[\\]/;

/**
 * Read a branch, tag or commit argument.
 *
 * Strict enough that the value cannot change what path it lands in - no `..`, no empty or
 * dotted segment, nothing git itself refuses - and loose enough to take any name git allows.
 *
 * @param {unknown} value
 * @param {string} field
 * @returns {string}
 */
export function refName(value, field) {
  const name = requiredText(value, field);
  const segments = name.split("/");
  if (
    name.length > 255 ||
    BAD_REF.test(name) ||
    name.includes("..") ||
    name.includes("@{") ||
    name.startsWith("-") ||
    name.endsWith(".") ||
    segments.some((segment) => segment === "" || segment.startsWith(".") || segment.endsWith(".lock"))
  ) {
    throw new Refused(`\`${field}\` '${name}' is not a valid branch, tag or commit name.`);
  }
  return name;
}

/**
 * A reference as it goes into a URL path. GitLab reads `feature/login` as one name, so it is one
 * path segment, the slash encoded, unlike a name in a query string.
 *
 * @param {string} name A name already accepted by `refName`.
 * @returns {string}
 */
export function refPath(name) {
  return encodeURIComponent(name);
}

/**
 * Read a path inside a repository.
 *
 * @param {unknown} value
 * @param {string} field
 * @param {{ allowEmpty?: boolean }} [options] `allowEmpty` makes the repository root a valid answer.
 * @returns {{ path: string, encoded: string }} The cleaned path, and the same as GitLab's files API
 *   wants it in a URL: one segment, the slashes encoded.
 */
export function filePath(value, field, { allowEmpty = false } = {}) {
  const raw = rawText(value, field) ?? "";
  const trimmed = raw.trim().replace(/^\/+/, "").replace(/\/+$/, "");
  if (trimmed === "") {
    if (allowEmpty) return { path: "", encoded: "" };
    throw new Refused(`\`${field}\` is required: a path inside the repository, such as 'src/index.js'.`);
  }
  const segments = trimmed.split("/");
  if (/[\u0000-\u001f\u007f]/.test(trimmed) || segments.some((s) => s === "" || s === "." || s === "..")) {
    throw new Refused(
      `\`${field}\` '${trimmed}' is not a path inside the repository: no empty, '.' or '..' segments.`,
    );
  }
  return { path: trimmed, encoded: encodeURIComponent(trimmed) };
}
