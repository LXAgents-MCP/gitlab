/** Keeping a secret out of text that is about to be shown or logged. */

/**
 * Remove a secret from text that is about to be shown.
 */
export function redact(text: unknown, secret: string | undefined): string {
  return secret ? String(text).split(secret).join("***") : String(text);
}
