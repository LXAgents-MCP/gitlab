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
