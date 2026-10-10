---
name: memory-tasks-two-token-auth
description: Separating the token a client presents to the GitLab MCP server (MCP_AUTH_TOKEN) from the key the server uses against GitLab (GITLAB_API_KEY); released as 0.3.0.
---

# Two tokens — GitLab

## 2026-10-10 — in progress

**Goal.** Two secrets with two jobs. One authenticates a client to this server; the other authenticates this
server to GitLab. 0.2.0 had one: over HTTP the client's bearer token was the GitLab token itself, and over stdio
`MCP_AUTH_TOKEN` held it.

**Objective.** Over HTTP every route but `/healthz` needs `Authorization: Bearer <MCP_AUTH_TOKEN>`, where
`MCP_AUTH_TOKEN` is a secret of at least 32 characters in the server's environment, compared in constant time;
HTTP will not start without it, as in `shared-instruction`. Every GitLab call, over stdio and over HTTP, is made
with `GITLAB_API_KEY`, read per call from the server's environment. The client's bearer is never forwarded and
never used against GitLab; stdio never reads `MCP_AUTH_TOKEN`. `npm test` is green and the wiki says how to set
both up.

**Detail.** Edit this repository only; `LXAgents-MCP/github` has the same change in its own record, made before
this one, with `GITHUB_API_KEY`. The 45 tools do not change. The reference is `shared-instruction`'s `src/auth.js`.
The names `GITHUB_API_KEY` and `GITLAB_API_KEY` are the owner's. This record supersedes `http-token-auth.md` in part.

**Status:** in progress.

## Tasks

| # | Title | Branch | PR |
|---|---|---|---|
| 1 | Task record | `chore/two-token-auth-plan` | |
| 2 | Separate the server token from the GitLab key | `feat/two-token-auth` | |
| 3 | Setup guide and corrected documentation | `docs/two-token-setup` | |
| 4 | Release | `release/0.3.0` | |

Branches stack: task 1 from `master`, task `k` from task `k-1`. The `PR` column is filled by a closing commit
once the pull requests exist.

## Decisions

The owner asked for the change and the names, and said to start after the plan. Where a choice was open the
recommended option was taken, per the standing instruction.

| # | Decision |
|---|---|
| T1 | Two secrets. `MCP_AUTH_TOKEN` is the server's own secret and guards the HTTP door; `GITLAB_API_KEY` is the GitLab token. Replaces D1 (the bearer token as the GitLab token) and D3 (HTTP never reads the environment) of `http-token-auth.md`. Declined: keeping the passthrough as an option, which is two ways to authenticate and a second path to test. |
| T2 | `MCP_AUTH_TOKEN` must be at least 32 characters and HTTP refuses to start without it, copied from `shared-instruction`. Replaces D4, which was about a token the server did not own; a secret the operator generates can have a floor. |
| T3 | stdio needs no client authentication, because a pipe has no header, and never reads `MCP_AUTH_TOKEN`. It reads `GITLAB_API_KEY`. |
| T4 | A missing `GITLAB_API_KEY` does not stop either transport. A tool refuses and names the variable; the HTTP process also says so on stderr at start. Reading per call is unchanged. |
| T5 | The bearer a client sends is compared and discarded. It is never sent to GitLab, never logged and never put in a result. The GitLab key is never taken from a request. |
| T6 | The comparison hashes both values with SHA-256 and compares the digests in constant time, so neither length nor a correct prefix shows in the timing. |
| T7 | The release is `0.3.0`: both variable names change again, which breaks 0.2.0 registrations. **Not approved by the owner in so many words**; chosen under the standing instruction and stated to them with the plan. No git tag, nothing published. |
| T8 | The name `MCP_AUTH_TOKEN` now means here what it means in `shared-instruction`. D10's hazard (a value meant for one server inherited by the other and sent to GitLab) is gone for stdio, which does not read it. What remains is two HTTP servers given one value, which the documentation advises against. |
| T9 | Consequence, recorded because the earlier decision rested on it: every holder of `MCP_AUTH_TOKEN` acts as the owner of `GITLAB_API_KEY`, `repo_delete` included, and there is no identity per caller. Mitigations: give the key the least scope that works, bind loopback by default, require a 32-character secret, terminate TLS in front, and rotate by restart. |

## Task entries

### Task 1 — chore/two-token-auth-plan

Landed: this record and its row in `memory-index.md`. The first commit of the branch fills the `PR` column of
`http-token-auth.md` for 0.2.0, which was left for a closing commit. Nothing outside `.agents/` changes in this
task. The `PR` column here is filled by a closing commit, not here, so no later branch needs a rebase.

Baseline before any change: `npm test` ran 292 passing and none failing.
