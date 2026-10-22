/**
 * The two secrets this server deals with, and which one is for what.
 *
 * - **`MCP_AUTH_TOKEN` guards the door.** It is the server's own secret for the HTTP transport: a
 *   client proves it holds it with `Authorization: Bearer <token>`, and the server refuses to
 *   serve HTTP without one. stdio is a pipe the client spawns on its own machine, so there is
 *   nobody to authenticate and `src/index.js` never reads it on that path.
 * - **`GITLAB_API_KEY` is the key to GitLab.** It is the token every GitLab call is made with, on
 *   both transports, read from this process's environment when a tool is called. A client never
 *   supplies it and never sees it, and the bearer a client presents is never sent to GitLab.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { createHash, timingSafeEqual } from "node:crypto";
import { AUTH_ENV, MIN_AUTH_TOKEN_LENGTH, TOKEN_ENV } from "../config/config.js";

/* ------------------------------------------------------------------ the door: MCP_AUTH_TOKEN */

/**
 * The server's access token, or an empty string when there is none.
 *
 * Read at call time and never at import, so a value set later is seen and a test can set and unset
 * it. Surrounding whitespace is dropped: a token copied out of a file or an `env_file` often
 * carries a trailing newline, and an HTTP header cannot.
 *
 * @returns {string}
 */
export function configuredAuthToken() {
  return (process.env[AUTH_ENV] ?? "").trim();
}

/**
 * Why `token` cannot guard the HTTP transport, or `null` when it can.
 *
 * The message names the variable and never the value, so it is safe to log.
 *
 * @param {string} [token]
 * @returns {string | null}
 */
export function authTokenProblem(token = configuredAuthToken()) {
  if (token === "") return `${AUTH_ENV} is not set`;
  if (token.length < MIN_AUTH_TOKEN_LENGTH) {
    return `${AUTH_ENV} is ${token.length} characters, and it must be at least ${MIN_AUTH_TOKEN_LENGTH}`;
  }
  return null;
}

/** SHA-256 of a string, so two values of any length compare in constant time. */
export function digest(value) {
  return createHash("sha256").update(value).digest();
}

/**
 * The credential in an `Authorization: Bearer <credential>` header, or `null`.
 *
 * The scheme is case-insensitive (RFC 7235). Anything else - another scheme, no credential, two of
 * them - is not a bearer credential. The pattern is linear: the space run and the credential are
 * disjoint character classes.
 *
 * @param {string | undefined} header
 * @returns {string | null}
 */
export function bearerCredential(header) {
  const match = /^Bearer +(\S+)$/i.exec(header ?? "");
  return match ? match[1] : null;
}

/**
 * Whether `credential` is the token whose digest is `expected`.
 *
 * The comparison is constant-time over fixed-length digests, so neither the token's length nor how
 * much of it a guess got right shows in how long the answer takes.
 *
 * @param {string} credential
 * @param {Buffer} expected `digest(token)` of a token that passed `authTokenProblem`.
 * @returns {boolean}
 */
export function credentialMatches(credential, expected) {
  return timingSafeEqual(digest(credential), expected);
}

/* ------------------------------------------------------------------ the key: GITLAB_API_KEY */

/** @returns {string} The GitLab token in `GITLAB_API_KEY`, trimmed, or an empty string. */
export function configuredApiKey() {
  return (process.env[TOKEN_ENV] ?? "").trim();
}

/**
 * The text to scrub from anything about to be shown: the GitLab key. Never used to make a call.
 *
 * @returns {string}
 */
export function tokenToScrub() {
  return configuredApiKey();
}
