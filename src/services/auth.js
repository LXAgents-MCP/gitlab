/**
 * The two secrets this server deals with, and which one is for what.
 *
 * - **`MCP_AUTH_TOKEN` guards the door.** It is the server's own secret for the HTTP transport: a
 *   client proves it holds it with `Authorization: Bearer <token>`, and a server install - one that
 *   listens where other machines can reach it - refuses to serve HTTP without one. A local install
 *   listens on loopback only and may leave it unset; the `Host` and `Origin` checks in
 *   `src/routes/http.js` then stand where the token would. stdio is a pipe the client spawns on its
 *   own machine, so there is nobody to authenticate and `src/index.js` never reads it on that path.
 * - **`GITLAB_API_KEY` is the key to GitLab.** It is the token every GitLab call is made with, on
 *   both transports, read from this process's environment when a tool is called. A client never
 *   supplies it and never sees it, and the bearer a client presents is never sent to GitLab.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { createHash, timingSafeEqual } from "node:crypto";
import { AUTH_ENV, MIN_AUTH_TOKEN_LENGTH, TOKEN_ENV } from "../config/config.js";
import { Refused } from "../utils/errors.js";

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

/* ------------------------------------------------------------------ local installs: loopback */

/**
 * Whether `name` is this machine: `localhost`, an address in `127.0.0.0/8`, or `::1`, with or without
 * the brackets a URL puts round an IPv6 address. Used for where the HTTP transport listens, which
 * decides whether it is a local install, and for the host a request says it is addressed to.
 *
 * Only these literal forms count. A name that merely resolves to loopback does not, because what it
 * resolves to can change after the check, which is how DNS rebinding works.
 *
 * @param {string | undefined} name
 * @returns {boolean}
 */
export function isLoopbackHost(name) {
  const host = String(name ?? "")
    .trim()
    .toLowerCase()
    .replace(/^\[(.*)\]$/, "$1")
    .replace(/\.$/, "");
  if (host === "localhost" || host === "::1") return true;
  // A dotted IPv4 address in 127.0.0.0/8, matched by hand so that only src/routes/http.js loads a
  // network module.
  const octets = /^127\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(host);
  return octets !== null && octets.slice(1).every((octet) => Number(octet) <= 255);
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

/**
 * The GitLab token a call is made with, read when the call is made.
 *
 * Read per call and not at startup, so a server without a token still starts and says what is
 * missing when a tool is used. It comes from `GITLAB_API_KEY` in this process's environment, set in
 * the `env` block of the client's configuration over stdio and in the server's own environment
 * over HTTP, and is the same on both: the bearer token an HTTP client presents authenticates it to
 * this server and is never used here (see `configuredAuthToken` above). It is never a tool argument, so it never
 * appears in a prompt or a transcript.
 *
 * @returns {string}
 */
export function requireToken() {
  const token = configuredApiKey();
  if (token) return token;
  throw new Refused(
    `${TOKEN_ENV} is not set. Add it to the \`env\` block of this server's entry in the ` +
      "client configuration, or to the environment of the HTTP server process, then restart it.",
  );
}
