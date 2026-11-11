/** Keeping a secret out of text that is about to be shown or logged. */


/**
 * Remove a secret from text that is about to be shown.
 *
 * @param {unknown} text
 * @param {string} secret
 * @returns {string}
 */
export function redact(text, secret) {
  return secret ? String(text).split(secret).join("***") : String(text);
}
