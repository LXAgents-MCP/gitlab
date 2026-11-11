/** Where the HTTP transport listens: `PORT` and `HOST`, read when called. */
import { AUTH_ENV, DEFAULT_HOST, DEFAULT_PORT, LOCAL_HOST } from "./config.js";

/**
 * Read `PORT` and `HOST`: 3000 when `PORT` is unset or blank, as in `shared-instruction`. When `HOST` is
 * unset or blank it follows the access token: `0.0.0.0` when `MCP_AUTH_TOKEN` is set, as in
 * `shared-instruction`, and `127.0.0.1` when it is not, so a local install needs neither. Unlike
 * its `Number(PORT) || 3000`, a `PORT` that is set and is not a port is an error and not a silent 3000,
 * and `0` is accepted and asks the system for a free port.
 */
export function httpSettings(env: NodeJS.ProcessEnv = process.env): { port: number; host: string } {
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
