/**
 * The GitLab client: addressing, requests, and the one place a failure becomes a message.
 *
 * Every tool reaches GitLab through `gitlab()`. Nothing else builds a URL, so nothing else can
 * let an argument steer a call at a different endpoint: a project, a reference and a file path
 * are validated and percent-encoded here and nowhere else.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { optionalInt, rawText, requiredText } from "./args.js";
import {
  API_BASE,
  API_TIMEOUT_MS,
  DEFAULT_PER_PAGE,
  MAX_PER_PAGE,
  MAX_RESPONSE_BYTES,
  TOKEN_ENV,
} from "./config.js";
import { Refused } from "./errors.js";
import { VERSION } from "./version.js";

const JSON_ACCEPT = "application/json";

/* ------------------------------------------------------------------ the token */

/**
 * Read the API token from the environment, at call time.
 *
 * Read per call and not at startup, so a server without a token still starts and says what is
 * missing when a tool is used. The token comes from the `env` block of the client's
 * configuration for this server; it is never an argument, so it never appears in a prompt or a
 * transcript.
 *
 * @param {string} [envName]
 * @returns {string}
 */
export function requireToken(envName = TOKEN_ENV) {
  const token = process.env[envName]?.trim();
  if (!token) {
    throw new Refused(
      `${envName} is not set. Add it to the \`env\` block of this server's entry in the ` +
        "client configuration, then restart the client.",
    );
  }
  return token;
}

/**
 * Remove a secret from text that is about to be shown.
 *
 * @param {unknown} text
 * @param {string} secret
 * @returns {string}
 */
export function redact(text, secret) {
  return secret ? String(text).split(secret).join("***") : String(text);
}

/* ------------------------------------------------------------------ addressing */

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

/* ------------------------------------------------------------------ paging */

/**
 * Read `page` and `per_page` from a tool's arguments.
 *
 * @param {{ page?: unknown, per_page?: unknown }} args
 * @returns {{ page: number, per_page: number }}
 */
export function pageQuery(args) {
  return {
    page: optionalInt(args.page, "page", { min: 1 }) ?? 1,
    per_page: optionalInt(args.per_page, "per_page", { min: 1, max: MAX_PER_PAGE }) ?? DEFAULT_PER_PAGE,
  };
}

/**
 * The paging fields of a list result: the page asked for, and whether GitLab has another.
 *
 * @param {{ page: number, per_page: number }} query
 * @param {Record<string, string>} headers The response headers, lowercased.
 * @returns {{ page: number, per_page: number, has_more: boolean }}
 */
export function paging(query, headers) {
  // Most answers carry `x-next-page`; the commits API, which sends no totals, is read by its link header too.
  const next = (headers["x-next-page"] ?? "") !== "" || /rel="next"/.test(headers.link ?? "");
  return { page: query.page, per_page: query.per_page, has_more: next };
}

/* ------------------------------------------------------------------ requests */

/**
 * Read a response body as text, refusing an answer larger than the client will read.
 *
 * @param {Response} response
 * @param {string} host
 * @returns {Promise<string>}
 */
async function readText(response, host) {
  if (!response.body) return "";
  const reader = response.body.getReader();
  const chunks = [];
  let size = 0;

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MAX_RESPONSE_BYTES) {
      await reader.cancel();
      throw new Refused(`The answer from ${host} is larger than ${MAX_RESPONSE_BYTES / 1_000_000} MB. Ask for less.`);
    }
    chunks.push(value);
  }
  return Buffer.concat(chunks).toString("utf8");
}

/**
 * Call a hosting API and return its status, decoded body and headers.
 *
 * Every failure to get an answer - no network, a refused connection, a timeout - becomes a
 * `Refused` the model can read and act on. A non-2xx answer is not a failure here: the caller
 * knows what each status means. `token` is only used to scrub error text; the caller puts the
 * credential in `headers`.
 *
 * @param {string} url
 * @param {{ method?: string, headers?: Record<string, string>, json?: unknown, token?: string, raw?: boolean }} [options]
 *   `raw` keeps the whole body of a successful answer as text instead of decoding JSON; an error answer is
 *   still decoded, so that its message can be read.
 * @returns {Promise<{ status: number, body: any, headers: Record<string, string> }>}
 */
export async function callApi(url, { method = "GET", headers = {}, json, token = "", raw = false } = {}) {
  const host = new URL(url).host;
  const init = {
    method,
    headers: json === undefined ? headers : { ...headers, "Content-Type": "application/json" },
    signal: AbortSignal.timeout(API_TIMEOUT_MS),
  };
  if (json !== undefined) init.body = JSON.stringify(json);

  let response;
  let text;
  try {
    response = await fetch(url, init);
    text = await readText(response, host);
  } catch (error) {
    if (error instanceof Refused) throw error;
    if (error?.name === "TimeoutError" || error?.name === "AbortError") {
      throw new Refused(`${host} did not answer within ${API_TIMEOUT_MS / 1000} seconds. Try again.`);
    }
    const reason = redact(error?.cause?.message ?? error?.message ?? error, token);
    throw new Refused(`Could not reach ${host}: ${reason}. Check the network connection.`);
  }

  let body = null;
  if (text !== "") {
    if (raw && response.ok) {
      body = text;
    } else {
      try {
        body = JSON.parse(text);
      } catch {
        body = text.slice(0, 300);
      }
    }
  }
  return { status: response.status, body, headers: Object.fromEntries(response.headers) };
}

/**
 * What GitLab said went wrong, as a short suffix: ": message (detail; detail)".
 *
 * GitLab answers in four shapes: a plain string (`{"message": "404 Project Not Found"}`), an
 * object of field errors (`{"message": {"name": ["has already been taken"]}}`), a list of
 * messages, and an OAuth-style `error` pair.
 *
 * @param {any} body
 * @returns {string}
 */
export function apiDetail(body) {
  if (typeof body === "string") {
    // A security check or an outage answers with a page of HTML, which says nothing a reader can use.
    if (/^\s*<(!doctype|html)/i.test(body)) return ": the answer was a web page and not the API's, such as a security check or an outage";
    return body ? `: ${body}` : "";
  }
  if (!body || typeof body !== "object") return "";

  const { message, error, error_description: description } = body;
  if (typeof message === "string") return `: ${message}`;
  if (Array.isArray(message)) return message.length > 0 ? `: ${message.join("; ")}` : "";
  if (message && typeof message === "object") {
    const fields = Object.entries(message).map(([field, problems]) => `${field} ${[].concat(problems).join(", ")}`);
    return fields.length > 0 ? `: ${fields.join("; ")}` : "";
  }
  if (typeof error === "string") return `: ${error}` + (description ? ` (${description})` : "");
  return "";
}

/** When the limit lifts, in words the model can act on. */
function whenToRetry(headers) {
  const seconds = Number(headers["retry-after"]);
  if (Number.isFinite(seconds) && seconds > 0) return `Wait ${Math.ceil(seconds)} seconds and try again.`;

  const reset = Number(headers["ratelimit-reset"]);
  if (Number.isFinite(reset) && reset > 0) {
    const minutes = Math.max(1, Math.ceil((reset * 1000 - Date.now()) / 60_000));
    return `The limit resets at ${new Date(reset * 1000).toISOString()}, in about ${minutes} minute${minutes === 1 ? "" : "s"}.`;
  }
  return "Wait a minute and try again.";
}

/**
 * A refusal that says what to do next, for a status GitLab answered with.
 *
 * `explain` lets a tool word the statuses it knows better than the general case. It receives
 * the status and GitLab's own detail suffix, and returns a message, or `undefined` to fall
 * through to the general wording below.
 *
 * @returns {Refused}
 */
function failure({ status, body, headers, what, explain, token }) {
  const detail = apiDetail(body);
  let message = explain?.(status, detail);

  if (message === undefined) {
    if (status === 401) {
      message = `GitLab rejected the token (401). Check that ${TOKEN_ENV} holds a valid token that has not expired.`;
    } else if (status === 429) {
      message = `GitLab is rate limiting this token (429)${detail}. ${whenToRetry(headers)}`;
    } else if (status === 403) {
      message =
        `GitLab refused to ${what} (403)${detail}. The token lacks permission for this: it needs the \`api\` ` +
        "scope (`read_api` is enough only for reads), and its user needs a role in the project that allows it.";
    } else if (status === 404) {
      message =
        `GitLab could not find what it needs to ${what} (404)${detail}. Check the spelling. GitLab ` +
        "also answers 404 for a private project the token cannot see.";
    } else if (status === 405) {
      message =
        `GitLab refused to ${what} (405)${detail}. The method is not allowed here; for a project that was ` +
        "renamed or moved, use its current path or its numeric id.";
    } else if (status === 409) {
      message = `GitLab could not ${what} (409)${detail}. Something changed or conflicts; read it again before retrying.`;
    } else if (status === 400 || status === 422) {
      message = `GitLab could not ${what} (${status})${detail}.`;
    } else if (status >= 500) {
      message = `GitLab failed to ${what} (${status})${detail}. Try again shortly.`;
    } else {
      message = `GitLab answered ${status}${detail} when asked to ${what}.`;
    }
  }
  return new Refused(redact(message, token));
}

/**
 * Call GitLab and return the answer, or refuse with a message that names the next step.
 *
 * A 2xx answer is returned. So is any status listed in `allow`, for a call where that status is
 * an answer and not a failure - a tag that does not exist, for one. Everything else is refused.
 *
 * The token goes in an `Authorization: Bearer` header, which GitLab accepts for personal, project
 * and group access tokens. The runtime drops that header, and no other, when a redirect changes
 * host; GitLab's own `PRIVATE-TOKEN` header would be forwarded.
 *
 * @param {string} method
 * @param {string} path Starts with `/` and is built only from `parseRepo`, `refPath`, `filePath`
 *   and numbers, so it needs no further encoding. No query string: use `query`.
 * @param {{
 *   query?: Record<string, unknown>,
 *   json?: unknown,
 *   accept?: string,
 *   allow?: number[],
 *   what?: string,
 *   explain?: (status: number, detail: string) => string | undefined,
 *   raw?: boolean,
 * }} [options] `what` completes "... to <what>" in a refusal.
 * @returns {Promise<{ status: number, body: any, headers: Record<string, string> }>}
 */
export async function gitlab(method, path, options = {}) {
  const { query, json, accept = JSON_ACCEPT, allow = [], what = "complete the request", explain, raw = false } = options;
  if (!path.startsWith("/") || /[?#]/.test(path)) {
    throw new Error(`gitlab(): path must start with '/' and carry no query or fragment, got '${path}'.`);
  }

  const token = requireToken();
  const url = new URL(`${API_BASE}${path}`);
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== null && value !== "") url.searchParams.set(key, String(value));
  }

  const answer = await callApi(url.href, {
    method,
    token,
    json,
    raw,
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: accept,
      "User-Agent": `lxagents-mcp-gitlab/${VERSION}`,
    },
  });

  if ((answer.status >= 200 && answer.status < 300) || allow.includes(answer.status)) return answer;
  throw failure({ ...answer, what, explain, token });
}
