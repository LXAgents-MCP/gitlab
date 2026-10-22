/** Small text helpers shared by the tools. Pure functions, no I/O. */

/** The abbreviated form of a commit sha. */
export function short(sha) {
  return typeof sha === "string" ? sha.slice(0, 7) : "";
}

/** The first line of a message, which is all a listing needs. */
export function firstLine(message) {
  return typeof message === "string" ? message.split(/\r?\n/, 1)[0] : "";
}

/**
 * Decode bytes as text: UTF-8 when they are valid UTF-8, Windows-1252 when they are not.
 *
 * The byte order mark is kept (`ignoreBOM`), so a caller that cares can strip it explicitly.
 *
 * @param {Uint8Array} bytes
 * @returns {string}
 */
export function decodeText(bytes) {
  let encoding = "utf-8";
  try {
    new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    encoding = "windows-1252";
  }
  return new TextDecoder(encoding, { ignoreBOM: true }).decode(bytes);
}

/**
 * Cut text to a number of characters, never leaving half of a surrogate pair at the cut.
 *
 * @param {string} text
 * @param {number} max
 * @returns {[string, boolean]} The text and whether it was cut.
 */
export function cutText(text, max) {
  if (text.length <= max) return [text, false];
  let end = max;
  const last = text.charCodeAt(end - 1);
  if (last >= 0xd800 && last <= 0xdbff) end -= 1;
  return [text.slice(0, end), true];
}

/** Does this look like binary data? A NUL byte early on is the usual tell. */
export function looksBinary(bytes) {
  return bytes.subarray(0, 8000).includes(0);
}

/**
 * Cut text to a length, keeping the start.
 *
 * @param {string} text
 * @param {number} max
 * @returns {[string, boolean]} The text and whether it was cut.
 */
export function head(text, max) {
  return text.length > max ? [`${text.slice(0, max)}\n... (truncated)`, true] : [text, false];
}

/**
 * Cut text to a length, keeping the end - where a log says why it failed.
 *
 * @param {string} text
 * @param {number} max
 * @returns {[string, boolean]} The text and whether it was cut.
 */
export function tail(text, max) {
  return text.length > max ? [`(truncated) ...\n${text.slice(text.length - max)}`, true] : [text, false];
}

/**
 * A job's log as text to read: one kind of line break, and none of the colour codes that tools
 * write for a terminal, or of the `section_start` and `section_end` markers GitLab's runner adds to
 * fold a log in its web page, which only add noise.
 *
 * @param {string} text
 * @returns {string}
 */
export function cleanLog(text) {
  return text
    .replace(/^\uFEFF/, "")
    .replace(/section_(?:start|end):\d+:[^\r\n\u001b]*\r?/g, "")
    .replace(/\r\n?/g, "\n")
    .replace(/\u001b\[[0-9;]*[A-Za-z]/g, "");
}
