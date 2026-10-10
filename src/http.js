#!/usr/bin/env node
/**
 * The HTTP transport: `POST /mcp`, and `GET /healthz`.
 *
 * Every route but `GET /healthz` needs `Authorization: Bearer <token>`, and the token is the GitLab
 * token (`src/auth.js`). It is checked for shape before anything else is read - before the body is
 * parsed and before a path is matched, so a caller without a token cannot make this server read
 * 4 MB or find out which routes exist - and it is used for the GitLab calls of that one request.
 * Nothing is stored between requests: each one gets its own `McpServer`, closed when its response
 * is.
 *
 * This file is the entry point `npm run start:http` runs, and `src/index.js` reaches it when
 * `MCP_TRANSPORT=http`. It binds `127.0.0.1` unless `HOST` says otherwise, because what crosses
 * the wire is a GitLab credential; whatever exposes it further must terminate TLS.
 *
 * Diagnostics go to stderr, as everywhere in this server.
 */
import { createServer as createHttpServer } from "node:http";
import { pathToFileURL } from "node:url";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { redact } from "./api.js";
import { bearerCredential, configuredToken, credentialProblem, enterHttpMode, runWithToken } from "./auth.js";
import { DEFAULT_HOST, DEFAULT_PORT, MAX_BODY_BYTES, TOKEN_ENV } from "./config.js";
import { log } from "./log.js";
import { createServer } from "./server.js";
import { SERVER_NAME, VERSION } from "./version.js";

/**
 * Send a JSON-RPC error: the envelope every other refusal on this transport uses, so a client never
 * has to branch on content type to find out it was refused.
 */
function rpcError(res, status, code, message, headers = {}) {
  const body = JSON.stringify({ jsonrpc: "2.0", error: { code, message }, id: null });
  res.writeHead(status, { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body), ...headers });
  res.end(body);
}

/** The refusal for a request with no usable token. Neither answer says anything about a token's value. */
function unauthorized(res, presented) {
  rpcError(
    res,
    401,
    -32001,
    presented
      ? "Unauthorized: the token is not valid"
      : "Unauthorized: send your GitLab token as Authorization: Bearer <token>",
    { "WWW-Authenticate": presented ? 'Bearer realm="mcp", error="invalid_token"' : 'Bearer realm="mcp"' },
  );
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
 */
async function handle(req, res) {
  const path = new URL(req.url ?? "/", "http://localhost").pathname;

  // The one open route, and exact: the method and the path, nothing that merely starts with it.
  if (req.method === "GET" && path === "/healthz") {
    const body = JSON.stringify({ status: "ok", server: SERVER_NAME, version: VERSION });
    res.writeHead(200, { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body) });
    res.end(body);
    return;
  }

  const credential = bearerCredential(req.headers.authorization);
  if (credential === null) return unauthorized(res, false);
  if (credentialProblem(credential) !== null) return unauthorized(res, true);

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
  await runWithToken(credential, () => transport.handleRequest(req, res, parsed));
}

/**
 * Read `PORT` and `HOST`.
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
  return { port, host: (env.HOST ?? "").trim() || DEFAULT_HOST };
}

/**
 * Start listening.
 *
 * @param {{ port?: number, host?: string }} [options] Port 0 asks the system for a free one.
 * @returns {Promise<{ server: import("node:http").Server, port: number, host: string, close: () => Promise<void> }>}
 */
export function startHttpServer({ port = DEFAULT_PORT, host = DEFAULT_HOST } = {}) {
  enterHttpMode();

  const server = createHttpServer((req, res) => {
    handle(req, res).catch((error) => {
      // A crash is text nobody wrote for a reader, so it is scrubbed like any other. The token is
      // not in scope here, but it can be in the message, and it must reach neither the log nor the caller.
      const credential = bearerCredential(req.headers.authorization) ?? "";
      log(redact(`HTTP handler crashed:\n${error?.stack ?? error}`, credential));
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

  // Said out loud, because the variable that carries the token over stdio does nothing here, and a
  // leftover value is exactly what someone checking a startup log should see named.
  if (configuredToken() !== "") {
    log(`${TOKEN_ENV} is set in this process's environment and is ignored over HTTP; each request's bearer token is used.`);
  }

  let running;
  try {
    running = await startHttpServer(settings);
  } catch (error) {
    log(`${SERVER_NAME} ${VERSION} could not listen on ${settings.host}:${settings.port}: ${error.message}`);
    process.exitCode = 1;
    return;
  }

  log(`${SERVER_NAME} ${VERSION} serving over http on http://${running.host}:${running.port}/mcp`);
  log("a bearer token (your GitLab token) is required on every route except GET /healthz.");

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
