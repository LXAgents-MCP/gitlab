/**
 * The HTTP client under every GitLab call: one request with a timeout and a size cap, and every
 * failure to get an answer turned into a `Refused` the model can read.
 *
 * Nothing here writes to stdout: on stdio, stdout is the protocol channel.
 */
import { API_TIMEOUT_MS, MAX_RESPONSE_BYTES } from "../config/config.js";
import { Refused } from "../utils/errors.js";
import { redact } from "../utils/redact.js";

/**
 * Read a response body as text, refusing an answer larger than the client will read.
 *
 * @param {Response} response
 * @param {string} host
 * @returns {Promise<string>}
 */
async function readText(response, host) {
  if (!response.body) return "";
  const reader = response.body.getReader();
  const chunks = [];
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
 *
 * @param {string} url
 * @param {{ method?: string, headers?: Record<string, string>, json?: unknown, token?: string, raw?: boolean }} [options]
 *   `raw` keeps the whole body of a successful answer as text instead of decoding JSON; an error answer is
 *   still decoded, so that its message can be read.
 * @returns {Promise<{ status: number, body: any, headers: Record<string, string> }>}
 */
export async function callApi(url, { method = "GET", headers = {}, json, token = "", raw = false } = {}) {
  const host = new URL(url).host;
  const init = {
    method,
    headers: json === undefined ? headers : { ...headers, "Content-Type": "application/json" },
    signal: AbortSignal.timeout(API_TIMEOUT_MS),
  };
  if (json !== undefined) init.body = JSON.stringify(json);

  let response;
  let text;
  try {
    response = await fetch(url, init);
    text = await readText(response, host);
  } catch (error) {
    if (error instanceof Refused) throw error;
    if (error?.name === "TimeoutError" || error?.name === "AbortError") {
      throw new Refused(`${host} did not answer within ${API_TIMEOUT_MS / 1000} seconds. Try again.`);
    }
    const reason = redact(error?.cause?.message ?? error?.message ?? error, token);
    throw new Refused(`Could not reach ${host}: ${reason}. Check the network connection.`);
  }

  let body = null;
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
