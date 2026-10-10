# Connect a Client

How to register this server with an MCP client and give it a token. Four steps; the third is the one
people forget.

## 1. Create a token

On GitLab.com, create an access token with the `api` scope — or `read_api` if the agent should only
read. Prefer a **project or group access token**, where your plan offers them, so that the token
belongs to the one project or group the agent should touch, with the lowest role that does the job;
a personal access token acts as its owner everywhere. What each tool needs is in
[Environment](../environments/env.md#what-the-token-needs). Treat the token like a password: it can
do whatever you granted it.

## 2. Register the server

The package is private and is not published, so the client runs the server by path.
Clone the repository, run `npm install` once, then add the server to the client.

**Claude Desktop** — edit `claude_desktop_config.json`:

* Windows: `%APPDATA%\Claude\claude_desktop_config.json`
* Mac: `~/Library/Application Support/Claude/claude_desktop_config.json`

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

**Claude Code** — from a terminal:

```bash
claude mcp add lxagents-gitlab \
  --env LXAGENTS_MCP_GITLAB_API_KEY=<your GitLab token> \
  -- node /absolute/path/to/gitlab/src/index.js
```

Or commit a project `.mcp.json` that reads the token from the environment instead of holding it:

```json
{
  "mcpServers": {
    "lxagents-gitlab": {
      "command": "node",
      "args": ["/absolute/path/to/gitlab/src/index.js"],
      "env": { "LXAGENTS_MCP_GITLAB_API_KEY": "${LXAGENTS_MCP_GITLAB_API_KEY}" }
    }
  }
}
```

**In a container** instead of on the host: the same entry with `docker` as the command. See
[Docker](../environments/docker.md).

## 3. Restart the client

**Registering a server does not reach a session that is already running.** The client loads its
connector list at session start, so a server added mid-session reports healthy and is still absent
from the tool surface until the session restarts. That is the most common cause of "the tools are
not there", and it looks like a broken server rather than a stale session.

## 4. Check that it is there

Ask the client to list the server's tools, or run the MCP Inspector (`npm run inspect`). You should
see the tools in [MCP surface](../reference/mcp-surface.md), all taking a `repo` as a project's full
path or numeric id except `repo_create` and `repo_search`.

Then ask for something harmless and reversible, such as creating a private project with a throwaway
name:

> Create a private GitLab repository called `scratch-delete-me` in my namespace.

A rejected token comes back as a message naming `LXAGENTS_MCP_GITLAB_API_KEY`; a token that lacks a
permission comes back as a 403 that says so.

## Related pages

* [Environment](../environments/env.md)
* [Local setup](../environments/setup.md)
* [Docker](../environments/docker.md)
