import assert from "node:assert/strict";
import test from "node:test";
import { requireToken } from "../src/api.js";
import {
  bearerCredential,
  configuredToken,
  credentialProblem,
  currentToken,
  enterHttpMode,
  isHttpMode,
  requestScopedToken,
  runWithToken,
  tokenHome,
  tokenToScrub,
} from "../src/auth.js";
import { MAX_TOKEN_LENGTH } from "../src/config.js";
import { isRefusal, setToken, withToken, withoutToken, TOKEN } from "./helpers.js";

/*
 * Where the token comes from. Order matters in this file: `enterHttpMode` is a property of the
 * process and cannot be undone, so the test that sets it is the last one.
 */

test("bearerCredential reads the credential of a Bearer header and nothing else", () => {
  assert.equal(bearerCredential("Bearer abc"), "abc");
  assert.equal(bearerCredential("bearer    abc"), "abc", "the scheme is case-insensitive and tolerates a run of spaces");
  assert.equal(bearerCredential("BEARER abc"), "abc");

  for (const header of [undefined, "", "Bearer", "Bearer ", "Basic abc", "Bearer a b", "Token abc", "Bearerabc"]) {
    assert.equal(bearerCredential(header), null, JSON.stringify(header));
  }
});

test("credentialProblem checks the shape of a token and no more", () => {
  for (const token of ["glpat-abcdefghij0123456789", "ghp_abcdefghijklmnopqrstuvwxyz0123456789", "a".repeat(93), "x"]) {
    assert.equal(credentialProblem(token), null, token);
  }

  assert.match(credentialProblem("a".repeat(MAX_TOKEN_LENGTH + 1)), /longer than 512 characters/);
  assert.match(credentialProblem("tokén"), /not printable ASCII/);
  assert.match(credentialProblem("a b"), /not printable ASCII/);
  assert.match(credentialProblem(""), /not printable ASCII/);
});

test("over stdio the token is the environment's, trimmed", (t) => {
  setToken(t, `  ${TOKEN}  `);
  assert.equal(configuredToken(), TOKEN);
  assert.equal(currentToken(), TOKEN);
  assert.equal(requireToken(), TOKEN);
  assert.equal(tokenToScrub(), TOKEN);
  assert.equal(tokenHome(), "MCP_AUTH_TOKEN");

  setToken(t, "   ");
  assert.equal(currentToken(), null);
  assert.equal(tokenToScrub(), "");
});

test("a request's token wins over the environment inside the request, and only there", async (t) => {
  withToken(t);

  assert.equal(requestScopedToken(), undefined);
  const inside = await runWithToken("request-token", async () => {
    await new Promise((resolve) => setTimeout(resolve, 5));
    return { scoped: requestScopedToken(), current: currentToken(), required: requireToken() };
  });
  assert.deepEqual(inside, { scoped: "request-token", current: "request-token", required: "request-token" });
  assert.equal(currentToken(), TOKEN, "outside the request the environment is the source again");
});

test("requests that overlap each keep their own token", async () => {
  const tokens = Array.from({ length: 12 }, (_, i) => `token-${i}`);
  const seen = await Promise.all(
    tokens.map((token, i) =>
      runWithToken(token, async () => {
        await new Promise((resolve) => setTimeout(resolve, (12 - i) * 3));
        return requireToken();
      }),
    ),
  );
  assert.deepEqual(seen, tokens);
});

test("a process that serves HTTP never reads the environment's token", (t) => {
  withToken(t);
  assert.equal(isHttpMode(), false);

  enterHttpMode();

  assert.equal(isHttpMode(), true);
  assert.equal(currentToken(), null, "the environment is not a fallback");
  assert.equal(tokenToScrub(), "");
  assert.equal(tokenHome(), "the Authorization: Bearer header");
  assert.throws(() => requireToken(), isRefusal(/^This request carried no token\. Send your GitLab token as `Authorization: Bearer <token>`\.$/));
  assert.equal(runWithToken("t", () => requireToken()), "t");

  withoutToken(t);
  assert.throws(() => requireToken(), isRefusal(/carried no token/));
});
