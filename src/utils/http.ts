/** The HTTP transport's plumbing: its refusals, and reading a request body with a size cap. */
import type { IncomingMessage, OutgoingHttpHeaders, ServerResponse } from "node:http";
import { MAX_BODY_BYTES } from "../config/config.js";

/**
 * Send a JSON-RPC error: the envelope every other refusal on this transport uses, so a client never
 * has to branch on content type to find out it was refused.
 */
export function rpcError(
  res: ServerResponse,
  status: number,
  code: number,
  message: string,
  headers: OutgoingHttpHeaders = {},
): void {
  const body = JSON.stringify({ jsonrpc: "2.0", error: { code, message }, id: null });
  res.writeHead(status, { "Content-Type": "application/json", "Content-Length": Buffer.byteLength(body), ...headers });
  res.end(body);
}

/**
 * The refusal for a request that does not carry the access token. A request that sent a credential
 * which did not match also gets `error="invalid_token"`, so a client can tell a missing header from
 * a wrong one. Neither answer says anything about the token itself.
 */
export function unauthorized(res: ServerResponse, presented: boolean): void {
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
 * Read a request body, refusing one larger than `MAX_BODY_BYTES`.
 */
export function readBody(req: IncomingMessage): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0;
    let refused = false;
    req.on("data", (chunk: Buffer) => {
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
