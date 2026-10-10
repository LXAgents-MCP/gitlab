# gitlab

An MCP server that gives an LLM access to **GitLab.com** — projects, branches, files, commits, tags,
releases, issues, merge requests and CI/CD pipelines — through the GitLab REST API.

- **Server id:** `lxagents-gitlab`
- **Package:** `@lxagents-mcp/gitlab` — private; not published to npm.
- **Transport:** stdio only. The client starts the server as a subprocess and talks to it through a
  pipe. There is no network listener and none should be added: the token this server holds can
  change and delete projects.
- **Surface:** one tool per module in `src/tools/`, named by what it acts on and what it does
  (`repo_create`, `branch_list`, `pr_merge`, …). Every tool that acts on a repository takes one
  `repo` argument: the project's full path, such as `group/project`, or its numeric id. The names
  are the GitHub server's, with GitLab's words underneath: a repository is a project, a pull
  request (`pr_`) is a merge request, and a workflow (`workflow_`) is a pipeline. The list is in
  [MCP surface](wiki/reference/mcp-surface.md).
- **Token:** `LXAGENTS_MCP_GITLAB_API_KEY`, in the `env` block of the client's configuration for
  this server. It is read when a tool is called and is never an argument.
- **Requirements:** Node >= 20. ESM, no build step.

## Key features

- **One module, one tool.** A tool is a file in `src/tools/` named after it; the registry finds it
  and nothing else needs editing. A test holds files and tools together in both directions.
- **One place builds URLs.** A project, a branch or tag name and a file path are validated and
  percent-encoded in `src/api.js`, so an argument cannot steer a call at another endpoint.
- **The token never leaves its header.** It is not an argument, it is read per call, and it is
  removed from every message the server can produce — refusals and crashes alike.
- **Honest annotations.** Every tool says whether it reads, creates, changes or destroys, and a test
  holds it to that, so a client can ask a person first before anything destructive. `repo_delete`
  also refuses unless it is told the project's full path.
- **Failures that say what to do next.** A rejected token, a missing permission, a rate limit with
  its reset time, a name already taken: each is a sentence, not a status code.
- **Bounded output.** Lists page at up to 100; a file, a log or a diff is cut at a stated limit and
  says it was cut.
- **Nothing leaves the machine in the tests.** `fetch` is replaced and any request a test did not
  expect fails it; the one exception, a redirect check, talks to two servers on loopback.

## Quick start

```bash
npm install
npm test
```

Serve it to an MCP client over stdio, with the token in the environment:

```bash
LXAGENTS_MCP_GITLAB_API_KEY=<token> npm start
```

Or drive it by hand with the MCP Inspector:

```bash
LXAGENTS_MCP_GITLAB_API_KEY=<token> npm run inspect
```

Full instructions are in [`wiki/environments/setup.md`](wiki/environments/setup.md).

## Connect it

Add it to the client's configuration, with the token in the `env` block, then **restart the
client**. For Claude Desktop, in `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "lxagents-gitlab": {
      "command": "node",
      "args": ["/absolute/path/to/gitlab/src/index.js"],
      "env": { "LXAGENTS_MCP_GITLAB_API_KEY": "<your GitLab token>" }
    }
  }
}
```

**Registering a server does not reach a session that is already running.** A client loads its
connector list at session start, so a server added mid-session reports healthy and is still absent
from the tool surface until the session restarts. See
[Connect a client](wiki/guides/connect-a-client.md) for Claude Code and what the token needs.

## Documentation

- [Overview](wiki/information/overview.md) — what this serves and why it is its own server.
- [Architecture](wiki/information/architecture.md) — how a call becomes a request and a result.
- [MCP surface](wiki/reference/mcp-surface.md) — every tool, and what the server does not expose.
- [Local setup](wiki/environments/setup.md) — running and testing it.
- [Environment](wiki/environments/env.md) — the one variable, and what the token needs.
- [Connect a client](wiki/guides/connect-a-client.md) — Claude Desktop and Claude Code.
- [Security model](wiki/security/security-model.md) — the token, the surfaces that touch it, and
  what is deliberately not protected.

## Working with agents

This repository follows the organization's shared agent instruction set, read through a connector
and never copied here. Start at [`AGENTS.md`](AGENTS.md).

## License

MIT — see [`LICENSE`](LICENSE).
