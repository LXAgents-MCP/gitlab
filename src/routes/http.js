#!/usr/bin/env node
/**
 * The HTTP transport: `POST /mcp`, and `GET /healthz`.
 *
 * Two kinds of install, told apart by where the server listens:
 *
 * - **A server install** listens where other machines can reach it (`0.0.0.0`, or any `HOST` that
 *   is not loopback). Every route but `GET /healthz` needs `Authorization: Bearer <token>`, and the
 *   token is this server's own access token, `MCP_AUTH_TOKEN` (`src/services/auth.js`). It is a
 *   secret between the client and this server: it is checked before anything else is read - before
 *   the body is parsed and before a path is matched, so a caller without it cannot make this server
 *   read 4 MB or find out which routes exist - and it is never sent to GitLab. The server refuses to
 *   start without an access token, rather than serving an open door.
 * - **A local install** listens on loopback only, and may leave `MCP_AUTH_TOKEN` unset: nothing beyond
 *   this machine can reach it. With no `HOST` and no token it is what the server does. A web page
 *   open on this machine still can, through DNS rebinding, so without a token a request must say it
 *   is addressed to loopback in `Host`, and in `Origin` when it sends one, before anything is read.
 *   A token that is set is required here as well.
 *
 * The calls to GitLab are made with `GITLAB_API_KEY` from this process's environment, whoever the caller is.
 * Nothing is stored between requests: each one gets its own `McpServer`, closed when its response
 * is.
 *
 * This file is the entry point `npm run start:http` runs, and `src/index.js` reaches it when
 * `MCP_TRANSPORT=http`. It listens on port 3000, on all interfaces when an access token is set, as
 * `shared-instruction` does, and on 127.0.0.1 when none is, unless `PORT` and `HOST` say otherwise. It
 * holds a GitLab key, so what guards a server install is the access token it refuses to start without;
 * whatever exposes it beyond a trusted network must terminate TLS.
 *
 * Diagnostics go to stderr, as everywhere in this server.
 */
import { createServer as createHttpServer } from "node:http";
import { pathToFileURL } from "node:url";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { redact } from "../services/api.js";
import {
  authTokenProblem,
  bearerCredential,
  configuredApiKey,
  configuredAuthToken,
  credentialMatches,
  digest,
  isLoopbackHost,
} from "../services/auth.js";
import {
  AUTH_ENV,
  DEFAULT_HOST,
  DEFAULT_PORT,
  LOCAL_HOST,
  MAX_BODY_BYTES,
  MIN_AUTH_TOKEN_LENGTH,
  TOKEN_ENV,
} from "../config/config.js";
import { log } from "../utils/log.js";
import { createServer } from "./server.js";
import { SERVER_NAME, VERSION } from "../config/version.js";

/**
 * Send a JSON-RPC error: the envelope every other refusal on this transport uses, so a client never
 * has to branch on content type to find out it was refused.
 */
function rpcError(res, status, code, message, headers = {}) {
  const body = JSON.stringify({ jsonrpc: "2.0", error: { code, message }, id: null });
  res.writeHead(status, { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body), ...headers });
  res.end(body);
}

/**
 * The refusal for a request that does not carry the access token. A request that sent a credential
 * which did not match also gets `error="invalid_token"`, so a client can tell a missing header from
 * a wrong one. Neither answer says anything about the token itself.
 */
function unauthorized(res, presented) {
  rpcError(
    res,
    401,
    -32001,
    presented
      ? "Unauthorized: the token is not valid"
      : "Unauthorized: send the server's access token as Authorization: Bearer <token>",
    { "WWW-Authenticate": presented ? 'Bearer realm="mcp", error="invalid_token"' : 'Bearer realm="mcp"' },
  );
}

/**
 * Why a request to a local install - loopback, no access token - is refused, or `null` when it is not.
 *
 * Without a token, what keeps a web page from driving this server is that a browser always says
 * where a request is going: `Host` is the name the page used, and `Origin` is the page itself. A
 * rebound name or another site shows up in one of them. A client that is not a browser sends a
 * loopback `Host` and no `Origin`, and passes. The `Host` value is matched whole, as `name` or
 * `name:port`, so nothing in it can make a URL parser read a different host.
 *
 * @param {import("node:http").IncomingMessage} req
 * @returns {string | null}
 */
export function localRequestProblem(req) {
  const host = /^(\[[0-9a-f:.]+\]|[a-z0-9.-]+)(?::\d{1,5})?$/i.exec(req.headers.host ?? "");
  if (!host || !isLoopbackHost(host[1])) {
    return "Forbidden: this server has no access token, so it only answers requests addressed to localhost, 127.0.0.1 or [::1]";
  }
  const origin = req.headers.origin;
  if (origin !== undefined) {
    let name = null;
    try {
      const url = new URL(origin);
      if (url.protocol === "http:" || url.protocol === "https:") name = url.hostname;
    } catch {
      name = null;
    }
    if (!isLoopbackHost(name ?? "")) {
      return "Forbidden: this server has no access token, so it does not answer a web page from another origin";
    }
  }
  return null;
}

/**
 * Read a request body, refusing one larger than `MAX_BODY_BYTES`.
 *
 * @param {import("node:http").IncomingMessage} req
 * @returns {Promise<string>}
 */
function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    let refused = false;
    req.on("data", (chunk) => {
      if (refused) return;
      size += chunk.length;
      if (size > MAX_BODY_BYTES) {
        refused = true;
        req.pause();
        reject(Object.assign(new Error("request body too large"), { status: 413 }));
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => !refused && resolve(Buffer.concat(chunks).toString("utf8")));
    req.on("error", reject);
  });
}

/**
 * Answer one request.
 *
 * @param {import("node:http").IncomingMessage} req
 * @param {import("node:http").ServerResponse} res
 * @param {Buffer | null} expected `digest()` of the access token a request must carry, or `null` for a
 *   local install without one, where the request must be addressed to loopback instead.
 */
async function handle(req, res, expected) {
  const path = new URL(req.url ?? "/", "http://localhost").pathname;

  // The one open route, and exact: the method and the path, nothing that merely starts with it.
  if (req.method === "GET" && path === "/healthz") {
    const body = JSON.stringify({ status: "ok", server: SERVER_NAME, version: VERSION });
    res.writeHead(200, { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body) });
    res.end(body);
    return;
  }

  if (expected === null) {
    const problem = localRequestProblem(req);
    if (problem) return rpcError(res, 403, -32001, problem);
  } else {
    const credential = bearerCredential(req.headers.authorization);
    if (credential === null) return unauthorized(res, false);
    if (!credentialMatches(credential, expected)) return unauthorized(res, true);
  }

  if (path !== "/mcp") return rpcError(res, 404, -32000, "Not found");
  if (req.method !== "POST") return rpcError(res, 405, -32000, "Method not allowed: use POST", { Allow: "POST" });

  let parsed;
  try {
    parsed = JSON.parse(await readBody(req));
  } catch (error) {
    if (error?.status === 413) {
      return rpcError(res, 413, -32000, "Request body too large", { Connection: "close" });
    }
    return rpcError(res, 400, -32700, "Parse error: the body is not JSON");
  }

  const server = createServer();
  const transport = new StreamableHTTPServerTransport({ sessionIdGenerator: undefined, enableJsonResponse: true });
  res.on("close", () => {
    transport.close().catch(() => {});
    server.close().catch(() => {});
  });

  await server.connect(transport);
  await transport.handleRequest(req, res, parsed);
}

/**
 * Read `PORT` and `HOST`: 3000 when `PORT` is unset or blank, as in `shared-instruction`. When `HOST` is
 * unset or blank it follows the access token: `0.0.0.0` when `MCP_AUTH_TOKEN` is set, as in
 * `shared-instruction`, and `127.0.0.1` when it is not, so a local install needs neither. Unlike
 * its `Number(PORT) || 3000`, a `PORT` that is set and is not a port is an error and not a silent 3000,
 * and `0` is accepted and asks the system for a free port.
 *
 * @param {NodeJS.ProcessEnv} [env]
 * @returns {{ port: number, host: string }}
 */
export function httpSettings(env = process.env) {
  const raw = (env.PORT ?? "").trim();
  let port = DEFAULT_PORT;
  if (raw !== "") {
    port = Number(raw);
    if (!Number.isInteger(port) || port < 0 || port > 65535) {
      throw new Error(`PORT must be a whole number from 0 to 65535, got '${raw}'.`);
    }
  }
  const token = (env[AUTH_ENV] ?? "").trim();
  return { port, host: (env.HOST ?? "").trim() || (token === "" ? LOCAL_HOST : DEFAULT_HOST) };
}

/**
 * Start listening.
 *
 * @param {{ port?: number, host?: string, authToken?: string }} [options] Port 0 asks the system for
 *   a free one. `authToken` is the access token requests must carry; it defaults to `MCP_AUTH_TOKEN`.
 *   It may be empty only when `host` is loopback, a local install; otherwise it must pass
 *   `authTokenProblem`, or the promise rejects and nothing listens.
 * @returns {Promise<{ server: import("node:http").Server, port: number, host: string, close: () => Promise<void> }>}
 */
export function startHttpServer({ port = DEFAULT_PORT, host = DEFAULT_HOST, authToken = configuredAuthToken() } = {}) {
  const local = authToken === "" && isLoopbackHost(host);
  if (!local) {
    const problem = authTokenProblem(authToken);
    if (problem) return Promise.reject(new Error(problem));
  }
  const expected = local ? null : digest(authToken);

  const server = createHttpServer((req, res) => {
    handle(req, res, expected).catch((error) => {
      // A crash is text nobody wrote for a reader, so it is scrubbed like any other. None of the
      // secrets is in scope here, but any can be in the message, and none may reach the log or the
      // caller: the GitLab key, the access token, and whatever credential this request presented.
      const text = `HTTP handler crashed:\n${error?.stack ?? error}`;
      const presented = bearerCredential(req.headers.authorization) ?? "";
      log(redact(redact(redact(text, configuredApiKey()), authToken), presented));
      if (!res.headersSent) rpcError(res, 500, -32603, "Internal error");
      else res.end();
    });
  });

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, () => {
      server.off("error", reject);
      const address = server.address();
      resolve({
        server,
        host,
        port: typeof address === "object" && address ? address.port : port,
        close: () =>
          new Promise((done) => {
            const forced = setTimeout(() => server.closeAllConnections(), 5000);
            forced.unref();
            server.close(() => {
              clearTimeout(forced);
              done();
            });
            server.closeIdleConnections();
          }),
      });
    });
  });
}

/** Run the HTTP transport as this process's job: read the settings, listen, and close on a signal. */
export async function main() {
  let settings;
  try {
    settings = httpSettings();
  } catch (error) {
    log(error.message);
    process.exitCode = 2;
    return;
  }

  // A local install - loopback, and no token - needs none. Anything else fails closed, before anything
  // listens: HTTP without an access token where other machines can reach it is an open door on a
  // network, and the only thing to do about it is not to start. A token that is set is always checked.
  // `exitCode` and a return, not `process.exit()`: nothing is listening yet, so the process ends on its
  // own once the line is written.
  const local = configuredAuthToken() === "" && isLoopbackHost(settings.host);
  const problem = local ? null : authTokenProblem();
  if (problem) {
    log(
      `${SERVER_NAME} ${VERSION} will not start the HTTP transport on ${settings.host}: ${problem}. ` +
        `Other machines can reach that address, so set ${AUTH_ENV} to a secret of at least ` +
        `${MIN_AUTH_TOKEN_LENGTH} characters, for example the output of: openssl rand -hex 32. ` +
        "A local install needs no access token: leave HOST unset, or set it to 127.0.0.1, and leave " +
        `${AUTH_ENV} unset. stdio needs no access token.`,
    );
    process.exitCode = 1;
    return;
  }

  // Not fatal, as it is not over stdio: the server starts and a tool names the variable when it is
  // called. Said at start as well, because over HTTP nobody is watching the first call.
  if (configuredApiKey() === "") {
    log(`${TOKEN_ENV} is not set in this process's environment: every tool will refuse until it is.`);
  }

  let running;
  try {
    running = await startHttpServer(settings);
  } catch (error) {
    log(`${SERVER_NAME} ${VERSION} could not listen on ${settings.host}:${settings.port}: ${error.message}`);
    process.exitCode = 1;
    return;
  }

  const name = running.host.includes(":") ? `[${running.host}]` : running.host;
  const where = running.host === "0.0.0.0" ? `:${running.port}/mcp (all interfaces)` : `http://${name}:${running.port}/mcp`;
  log(`${SERVER_NAME} ${VERSION} serving over http on ${where}`);
  if (local) {
    log(
      `local install: ${AUTH_ENV} is not set, so only this machine can reach the server and no access token ` +
        `is asked for. Requests must be addressed to localhost, 127.0.0.1 or [::1]. To serve other machines, ` +
        `set ${AUTH_ENV} and leave HOST unset or make it 0.0.0.0.`,
    );
  } else {
    log(`the access token in ${AUTH_ENV} is required on every route except GET /healthz.`);
  }

  let closing = false;
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.on(signal, async () => {
      // A second signal means the operator has stopped waiting.
      if (closing) process.exit(0);
      closing = true;
      await running.close();
      process.exit(0);
    });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  await main();
}
