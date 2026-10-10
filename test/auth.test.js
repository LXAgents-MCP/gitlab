import assert from "node:assert/strict";
import test from "node:test";
import { requireToken } from "../src/api.js";
import {
  authTokenProblem,
  bearerCredential,
  configuredApiKey,
  configuredAuthToken,
  credentialMatches,
  digest,
  tokenToScrub,
} from "../src/auth.js";
import { AUTH_ENV, MIN_AUTH_TOKEN_LENGTH, TOKEN_ENV } from "../src/config.js";
import { isRefusal, setEnv, setToken, withToken, withoutToken, TOKEN } from "./helpers.js";

/*
 * The two secrets and which is for what. MCP_AUTH_TOKEN guards the HTTP door; GITLAB_API_KEY is
 * the key to GitLab. Neither stands in for the other.
 */

const ACCESS = "0123456789abcdef".repeat(4);

test("the variables are the ones the owner named", () => {
  assert.equal(TOKEN_ENV, "GITLAB_API_KEY");
  assert.equal(AUTH_ENV, "MCP_AUTH_TOKEN");
  assert.equal(MIN_AUTH_TOKEN_LENGTH, 32);
});

test("bearerCredential reads the credential of a Bearer header and nothing else", () => {
  assert.equal(bearerCredential("Bearer abc"), "abc");
  assert.equal(bearerCredential("bearer    abc"), "abc", "the scheme is case-insensitive and tolerates a run of spaces");
  assert.equal(bearerCredential("BEARER abc"), "abc");

  for (const header of [undefined, "", "Bearer", "Bearer ", "Basic abc", "Bearer a b", "Token abc", "Bearerabc"]) {
    assert.equal(bearerCredential(header), null, JSON.stringify(header));
  }
});

test("the access token is read at call time and trimmed", (t) => {
  setEnv(t, AUTH_ENV, null);
  assert.equal(configuredAuthToken(), "");

  setEnv(t, AUTH_ENV, `  ${ACCESS}\n`);
  assert.equal(configuredAuthToken(), ACCESS);
});

test("authTokenProblem refuses a missing or short access token and names the variable, never the value", () => {
  assert.equal(authTokenProblem(""), "MCP_AUTH_TOKEN is not set");
  assert.equal(authTokenProblem("a".repeat(31)), "MCP_AUTH_TOKEN is 31 characters, and it must be at least 32");
  assert.equal(authTokenProblem("a".repeat(32)), null);
  assert.equal(authTokenProblem(ACCESS), null);
  assert.ok(!String(authTokenProblem("short-secret")).includes("short-secret"));
});

test("authTokenProblem reads the environment when given no token", (t) => {
  setEnv(t, AUTH_ENV, null);
  assert.equal(authTokenProblem(), "MCP_AUTH_TOKEN is not set");

  setEnv(t, AUTH_ENV, ACCESS);
  assert.equal(authTokenProblem(), null);
});

test("credentialMatches accepts the token and nothing that merely resembles it", () => {
  const expected = digest(ACCESS);

  assert.equal(credentialMatches(ACCESS, expected), true);
  for (const wrong of [ACCESS.slice(0, -1), ACCESS.slice(1), `${ACCESS}0`, ACCESS.toUpperCase(), "", "x", "a".repeat(5000)]) {
    assert.equal(credentialMatches(wrong, expected), false, wrong.slice(0, 20));
  }
});

test("the GitLab key is the one in GITLAB_API_KEY, read per call and trimmed", (t) => {
  setToken(t, `  ${TOKEN}  `);
  assert.equal(configuredApiKey(), TOKEN);
  assert.equal(requireToken(), TOKEN);
  assert.equal(tokenToScrub(), TOKEN);

  setToken(t, "   ");
  assert.equal(configuredApiKey(), "");
  assert.equal(tokenToScrub(), "");
  assert.throws(() => requireToken(), isRefusal(/^GITLAB_API_KEY is not set\. .*`env` block/));
});

test("MCP_AUTH_TOKEN is not a GitLab key: with only the access token set, a call is refused", (t) => {
  withoutToken(t);
  setEnv(t, AUTH_ENV, ACCESS);

  assert.equal(configuredApiKey(), "");
  assert.throws(() => requireToken(), isRefusal(/^GITLAB_API_KEY is not set\./));
});

test("GITLAB_API_KEY is not an access token: it does not satisfy the HTTP door", (t) => {
  withToken(t);
  setEnv(t, AUTH_ENV, null);

  assert.equal(configuredAuthToken(), "");
  assert.equal(authTokenProblem(), "MCP_AUTH_TOKEN is not set");
  assert.equal(credentialMatches(TOKEN, digest(ACCESS)), false);
});
