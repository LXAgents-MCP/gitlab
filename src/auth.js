/**
 * Where the GitLab token comes from, and the rules for the one that arrives over HTTP.
 *
 * Over stdio the client spawns this process, so there is no request to carry a credential and the
 * token is read from the environment, from `MCP_AUTH_TOKEN`. Over HTTP every request carries its
 * own, as `Authorization: Bearer <token>`, and that token is the GitLab token: it is used for the
 * GitLab calls of that request and kept nowhere. The server holds no secret of its own.
 *
 * **The two sources never mix.** A process that serves HTTP never reads `MCP_AUTH_TOKEN`: a
 * request without a token is refused, it does not fall back to the owner's. `enterHttpMode` makes
 * that a property of the process, so it does not depend on the request context having survived to
 * the point a tool asks for the token.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { AsyncLocalStorage } from "node:async_hooks";
import { MAX_TOKEN_LENGTH, TOKEN_ENV } from "./config.js";

/** The token of the request being answered, for the duration of that request. */
const requestToken = new AsyncLocalStorage();

let httpMode = false;

/** Called once by a process that serves HTTP. From then on the environment is never a source of the token. */
export function enterHttpMode() {
  httpMode = true;
}

/** @returns {boolean} Whether this process serves HTTP. */
export function isHttpMode() {
  return httpMode;
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
 * Why a credential cannot be a token, or `null` when it can be one.
 *
 * Only the shape is checked. GitLab's token formats change, and GitLab is the one that knows
 * whether a token is valid, so there is no length floor and no prefix list here - a floor of 32
 * characters, as `shared-instruction` has, would refuse a valid personal access token of 26.
 *
 * @param {string} credential
 * @returns {string | null}
 */
export function credentialProblem(credential) {
  if (credential.length > MAX_TOKEN_LENGTH) return `longer than ${MAX_TOKEN_LENGTH} characters`;
  if (!/^[!-~]+$/.test(credential)) return "not printable ASCII without spaces";
  return null;
}

/**
 * Run `fn` with `token` as the token of the request it answers.
 *
 * @template T
 * @param {string} token
 * @param {() => T} fn
 * @returns {T}
 */
export function runWithToken(token, fn) {
  return requestToken.run(token, fn);
}

/** @returns {string | undefined} The token of the request being answered, or `undefined` outside one. */
export function requestScopedToken() {
  return requestToken.getStore();
}

/** @returns {string} The token in `MCP_AUTH_TOKEN`, trimmed, or an empty string. Stdio only. */
export function configuredToken() {
  return (process.env[TOKEN_ENV] ?? "").trim();
}

/**
 * The token a call to GitLab is made with, or `null` when there is none to use.
 *
 * Over HTTP that is the request's and nothing else; over stdio it is the environment's.
 *
 * @returns {string | null}
 */
export function currentToken() {
  const scoped = requestScopedToken();
  if (scoped !== undefined) return scoped;
  if (httpMode) return null;
  return configuredToken() || null;
}

/**
 * The token to scrub from text about to be shown, or an empty string. Never used to make a call.
 *
 * @returns {string}
 */
export function tokenToScrub() {
  return currentToken() ?? "";
}

/** Where the token being used came from, in words for a refusal. */
export function tokenHome() {
  return httpMode ? "the Authorization: Bearer header" : TOKEN_ENV;
}
