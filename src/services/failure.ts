/**
 * The one place a status GitLab answered with becomes a message that names the next step.
 */
import { TOKEN_ENV } from "../config/config.js";
import { Refused } from "../utils/errors.js";
import { redact } from "../utils/redact.js";
import type { ApiBody } from "../models/api-body.js";

/**
 * What GitLab said went wrong, as a short suffix: ": message (detail; detail)".
 *
 * GitLab answers in four shapes: a plain string (`{"message": "404 Project Not Found"}`), an
 * object of field errors (`{"message": {"name": ["has already been taken"]}}`), a list of
 * messages, and an OAuth-style `error` pair.
 */
export function apiDetail(body: ApiBody): string {
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
    const fields = Object.entries(message).map(
      ([field, problems]) => `${field} ${([] as unknown[]).concat(problems).join(", ")}`,
    );
    return fields.length > 0 ? `: ${fields.join("; ")}` : "";
  }
  if (typeof error === "string") return `: ${error}` + (description ? ` (${description})` : "");
  return "";
}

/** When the limit lifts, in words the model can act on. */
function whenToRetry(headers: Record<string, string>): string {
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
 */
/** An answer GitLab refused, and what the refusal needs to word it. */
export interface FailedCall {
  status: number;
  body: ApiBody;
  headers: Record<string, string>;
  what: string;
  explain?: (status: number, detail: string) => string | undefined;
  token: string;
}

export function failure({ status, body, headers, what, explain, token }: FailedCall): Refused {
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
