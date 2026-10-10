---
name: security-boundaries
description: The security procedure for this repository — the isolation rule, what to check before touching the token path, and what to escalate.
---

# Security Boundaries

The agent-facing half of this repository's security posture. The facts — which surfaces touch the
token, how each is guarded, what to grant it — live once, in
[`../../../wiki/security/security-model.md`](../../../wiki/security/security-model.md). Read that
first. This page carries only what an agent must *do*, and the one rule that exists because agents
work across repositories and people mostly do not.

## A. The isolation rule

**A security context belongs to exactly one repository. Never carry one across.**

This is not a style preference. Security conclusions are the ones that look most transferable and
travel worst: "the path check is handled upstream", "auth is enforced at the gateway", "that input
is already validated", "there is nothing to protect here". Each of those may be true where you
learned it and false here, and the failure mode is silent — you skip a check because you remember
it being unnecessary somewhere else.

In practice, in a session that touches more than one repository:

* **State which repository a security claim is about, every time.** An unqualified claim is the one
  that migrates.
* **Never apply a mitigation from another repository without re-deriving it here.**
* **Never copy a threat model, a checklist or a security page between repositories.** If the
  content is genuinely universal it is a proposal for the shared set, not a copy.
* **Never assume another repository's posture applies here.** The shared instruction repository
  holds no credential and serves public text, *by design*. Carrying "there is nothing to protect
  here" into this repository, which holds a token that can delete projects, is the worst
  version of this mistake, and the easy one to make, because it feels like context.

The trigger row in [`AGENTS.md`](../../../AGENTS.md) loads this page on security work so the rule is
in front of you *before* the reasoning that would violate it, not after.

## B. Before you change these, check this

| About to touch… | Verify before committing |
|---|---|
| `src/auth.js`, `requireToken`, `gitlab()`, `callApi` or `failure()` in `src/api.js` | The GitLab key still goes only into the `Authorization` header built in `gitlab()`, and is never a caller's bearer. Every message built from GitLab's text or a network error still passes `redact`. The tests that make GitLab, the network and a handler echo the token still pass — and fail when you remove the redaction, which is worth trying once. The token still travels as `Authorization: Bearer`, never as GitLab's own `PRIVATE-TOKEN` header, which the runtime would forward to another host on a redirect; redirects are still followed by the runtime and not by hand (nothing sets `redirect` to `manual`), and the redirect test still shows a second host receiving no token. |
| `callTool` in `src/server.js` | The crash path still redacts both the log line and the returned text, with the GitLab key. |
| `src/http.js` or `src/auth.js` | A request without the access token is still a 401 before any body is read or path matched; the comparison is still over SHA-256 digests with `timingSafeEqual`; HTTP still refuses to start without `MCP_AUTH_TOKEN` of 32 characters or more; the access token still appears in no request to GitLab (`test/http.test.js` searches for it) and `MCP_AUTH_TOKEN` is still never read over stdio; the default bind is still `127.0.0.1`. Try the mutation once: make the comparison always true and watch five tests fail. |
| `parseRepo`, `refName`, `refPath`, `filePath`, or how any tool builds a path | Every path segment still comes from these; empty, `.` and `..` segments are still refused; `gitlab()` still refuses a path with `?` or `#`. |
| A tool's `inputSchema` | No key names a credential (the registry test fails it), and nothing lets a caller choose the host. |
| `API_BASE` in `src/config.js`, or any new constant that looks like a host | It is still a constant. |
| A destructive tool, or any tool's annotations | Its kind in `test/surface.js` is `destroy`, its description tells the client to confirm with the owner, and `repo_delete` still refuses without a matching `confirm`. |
| Dependencies in `package.json` | Still the MCP SDK and zod only (`npm ls --omit=dev --depth=0`). A third needs a decision record first. |
| Anything under `src/` that could open a socket or serve | ``grep -rln -e "node:http" -e "node:net" -e "node:cluster" -e express src`` still prints only `src/http.js`. |
| The `Dockerfile` or `.dockerignore` | `--ignore-scripts` survives, the file still ends as `USER node`, and there is still no `ENV`, `EXPOSE` or `ENTRYPOINT` line (``grep -n -e '^ENV' -e '^EXPOSE' -e '^ENTRYPOINT' Dockerfile`` prints nothing). `.env*` and `.mcp.json` stay in `.dockerignore`. |

## C. What to escalate rather than decide

Ask the user; do not resolve these on your own initiative.

* **Any transport other than stdio and the HTTP one, any listener outside `src/http.js`, any route
  beyond `POST /mcp` and `GET /healthz`, HTTP that starts without an access token or accepts a
  caller's own GitLab token, any use of the access token against GitLab, any storage of tokens between
  requests.** See
  [`../../memory/decisions/two-token-auth.md`](../../memory/decisions/two-token-auth.md).
* **A third secret variable, a base-URL setting, or any real token in a file, a fixture or a
  commit.** There are none; adding one is a decision, not an edit.
* **Loosening a guard to make something work** — accepting a raw URL from a caller, bypassing
  `parseRepo`, dropping `confirm` from `repo_delete`, relaxing a redaction. The guard is the
  feature; a test that needs it off is a test to rewrite.
* **A tool that can act on something its name does not say.** A name and an annotation are what a
  client's approval prompt is built from.
* **A finding in a released version.** It reaches every client the next time they start the server,
  so the fix is a release with a **Clients must** line, and the version needs the owner's approval.
* **A new dependency.** Each one is transitive attack surface.

## D. What is not a security finding here

Named because each one has cost, or would cost, a round of investigation:

* **A registered server that reports healthy but has no tools.** The client loads connectors at
  session start, so a server added mid-session is absent until the session restarts. A stale
  session, not a broken or hijacked server.
* **stdio having no authentication.** Deliberate. Asking for a token on stdio is a design change to
  argue, not a gap.
* **`GET /healthz` answering without the access token.** The one open route; it says only that a server is
  there.
* **`MCP_AUTH_TOKEN` having the same name as `shared-instruction`'s variable.** It means the same
  thing there: the secret that guards an HTTP door. Recorded in the decision; the hazard left is one
  value on two servers, which the setup guide says not to do.
* **Every holder of the access token acting as the key's owner.** The accepted cost of the two-token
  design, stated in the security model and the decision.
* **A 404 for a project that exists.** GitLab answers 404 for a private project the token
  cannot see.
