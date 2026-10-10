---
name: security-boundaries
description: The security procedure for this repository — the token it holds, the surfaces that touch it, and the checks each one needs.
---

# Security Boundaries

This server holds a GitLab token that can change and delete projects. The line worth keeping if
you keep nothing else: **the token reaches one header, built in one place, and nothing — an
argument, a log line, an error — may carry it anywhere else.** The facts (attack surface,
deployment, what is and is not a secret) live in
[`../../../wiki/security/security-model.md`](../../../wiki/security/security-model.md). This page
carries what an agent must *do*.

## In this project

* **The credential** is `LXAGENTS_MCP_GITLAB_API_KEY`, in the environment of the server process,
  put there by the `env` block of the client's configuration. `requireToken` in `src/api.js` reads
  it on every call.
* **The reach** is stdio only: the process is reachable by the client that spawned it and by no
  one else. Outbound traffic goes to `gitlab.com`, a constant in `src/config.js`, and only
  when a tool is called.
* **The authority** is whatever the owner granted the token. The server adds none and removes
  none, and it includes tools that delete and merge.
* **The isolation rule.** A security context belongs to exactly one repository; never carry one
  across. `shared-instruction` holds no credential and serves public text, so "there is nothing to
  protect here" is true there and false here — and it is the easy one to carry over, because it
  reads as context rather than as a claim. State which repository a security claim is about, every
  time. Never apply a mitigation from another repository without re-deriving it here, and never
  copy a threat model between repositories.

## Surfaces

### The token

| File | Exposure | Guard |
|---|---|---|
| `src/api.js` `requireToken` | Anyone who can set the process environment | Read per call; never an argument; the registry test fails any tool whose input names a credential |
| `src/api.js` `gitlab()` | The only place the `Authorization` header is built | One call site; the header goes to `callApi` and nowhere else |
| `src/api.js` `failure()` and `callApi` | Error text built from GitLab's answer or from the network | `redact` on every message; tests make GitLab echo the token and make the network error carry it |
| `src/server.js` `callTool` | The unexpected-exception path | `redact` on the log line and on the returned text; a test makes a handler throw the token |
| `src/log.js` | stderr | The logger writes what it is given and nothing logs a request or a header |

### Which endpoint a call reaches

| File | Exposure | Guard |
|---|---|---|
| `src/api.js` `parseRepo`, `refName`, `refPath`, `filePath` | Arguments from a model that may be steering on text it read | Validated and percent-encoded; empty, `.` and `..` segments refused; tests cover each |
| `src/api.js` `gitlab()` | The path handed to it | Must start with `/` and carry no `?` or `#`; a violation is a programming error, not a refusal |
| `src/config.js` `API_BASE` | The host | A constant: no argument or variable can change it |

## Verifying

| Check | How | Passing looks like |
|---|---|---|
| The token never appears in output | `npm test` — the api and server tests that make GitLab, the network and a handler echo it | all pass |
| No tool takes a credential | `npm test` — the registry test | passes |
| A path cannot leave its endpoint | `npm test` — the api tests for the four address helpers | pass |
| stdout stays silent | `npm test` — the stdio tests for `--help`, `--version` and a bad flag | stdout empty, exit as documented |
| No listener or HTTP library | ``grep -rn -e "node:http" -e "node:net" -e "node:cluster" -e express src`` | no output |
| Only two runtime dependencies | `npm ls --omit=dev --depth=0` | the MCP SDK and zod |

## Escalate, do not decide

Ask the user; do not resolve these on your own initiative.

* **Any transport other than stdio, any listener, any route.** See
  [`../../memory/decisions/stdio-only-transport.md`](../../memory/decisions/stdio-only-transport.md).
* **A second secret variable, a base-URL setting, or any real token in a file, a fixture or a
  commit.** There are none; adding one is a decision and not an edit.
* **Loosening a guard to make something work** — accepting a raw URL from a caller, bypassing
  `parseRepo`, dropping `confirm` from `repo_delete`, relaxing a redaction. The guard is the
  feature; a test that needs it off is a test to rewrite.
* **A tool that can act on something its name does not say.** A name and an annotation are what a
  client's approval prompt is built from.
* **A finding in a released version.** It reaches every client on their next start, so the fix is
  a release with a **Clients must** line, and the version needs the owner's approval.
* **A new dependency.** Each one is transitive attack surface.

## Not a finding here

* **stdio has no authentication.** Deliberate: the client spawns the process on its own machine, so
  the only caller is whoever already runs it. Asking for a token on stdio is a design change to
  argue, not a gap.
* **The token sitting in the client's `env` block.** That is the designed route. The alternative, an
  argument, would put it in a prompt and a transcript.
* **A registered server that reports healthy but has no tools.** The client loads connectors at
  session start, so a server added mid-session is absent until the session restarts: a stale
  session, not a broken or hijacked server.
* **A 404 for a project that exists.** GitLab answers 404 for a private project the token
  cannot see.

## Open

* Nothing in this repository has been run against live GitLab. Every request shape is checked
  against GitLab's documentation and against mocks, and the environment the work was done in could
  not reach `gitlab.com` with a token. A first smoke test against a throwaway repository is the
  owner's, and what a given token is actually allowed to do is only visible there.

## Summary

The server's whole security posture is one credential handled in one place. It never leaves
`src/api.js` except as an `Authorization` header to `gitlab.com`; it is scrubbed from every
message the server can produce; no argument can carry it or redirect a call to another host or
endpoint; and there is no network listener for anyone else to use it through. What is left is
authority: the token can do what the owner granted it, which is why the destructive tools are
annotated, why `repo_delete` asks to be told the repository's name, and why this page lists what to
ask the owner rather than decide.
