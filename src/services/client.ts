/**
 * The HTTP client under every GitLab call: one request with a timeout and a size cap, and every
 * failure to get an answer turned into a `Refused` the model can read.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { API_TIMEOUT_MS, MAX_RESPONSE_BYTES } from "../config/config.js";
import { Refused } from "../utils/errors.js";
import { redact } from "../utils/redact.js";
import type { ApiBody } from "../models/api-body.js";

/** An answer from a hosting API: its status, its decoded body and its headers, lowercased. */
export interface ApiAnswer {
  status: number;
  body: ApiBody;
  headers: Record<string, string>;
}

/** How `callApi` makes its one request. */
export interface CallOptions {
  method?: string;
  headers?: Record<string, string>;
  json?: unknown;
  token?: string;
  /**
   * Keeps the whole body of a successful answer as text instead of decoding JSON; an error answer is
   * still decoded, so that its message can be read.
   */
  raw?: boolean;
}

/**
 * Read a response body as text, refusing an answer larger than the client will read.
 */
async function readText(response: Response, host: string): Promise<string> {
  if (!response.body) return "";
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;

  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MAX_RESPONSE_BYTES) {
      await reader.cancel();
      throw new Refused(`The answer from ${host} is larger than ${MAX_RESPONSE_BYTES / 1_000_000} MB. Ask for less.`);
    }
    chunks.push(value);
  }
  return Buffer.concat(chunks).toString("utf8");
}

/**
 * Call a hosting API and return its status, decoded body and headers.
 *
 * Every failure to get an answer - no network, a refused connection, a timeout - becomes a
 * `Refused` the model can read and act on. A non-2xx answer is not a failure here: the caller
 * knows what each status means. `token` is only used to scrub error text; the caller puts the
 * credential in `headers`.
 */
export async function callApi(
  url: string,
  { method = "GET", headers = {}, json, token = "", raw = false }: CallOptions = {},
): Promise<ApiAnswer> {
  const host = new URL(url).host;
  const init: RequestInit = {
    method,
    headers: json === undefined ? headers : { ...headers, "Content-Type": "application/json" },
    signal: AbortSignal.timeout(API_TIMEOUT_MS),
  };
  if (json !== undefined) init.body = JSON.stringify(json);

  let response: Response;
  let text: string;
  try {
    response = await fetch(url, init);
    text = await readText(response, host);
  } catch (caught) {
    if (caught instanceof Refused) throw caught;
    const error = caught as { name?: string; message?: string; cause?: { message?: string } } | undefined;
    if (error?.name === "TimeoutError" || error?.name === "AbortError") {
      throw new Refused(`${host} did not answer within ${API_TIMEOUT_MS / 1000} seconds. Try again.`);
    }
    const reason = redact(error?.cause?.message ?? error?.message ?? error, token);
    throw new Refused(`Could not reach ${host}: ${reason}. Check the network connection.`);
  }

  let body: ApiBody = null;
  if (text !== "") {
    if (raw && response.ok) {
      body = text;
    } else {
      try {
        body = JSON.parse(text);
      } catch {
        body = text.slice(0, 300);
      }
    }
  }
  return { status: response.status, body, headers: Object.fromEntries(response.headers) };
}
