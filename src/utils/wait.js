/** Wait, in milliseconds. Kept apart so a test can replace the clock in one place. */
export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
