/** Wait, in milliseconds. Kept apart so a test can replace the clock in one place. */
export function sleep(ms: number): Promise<void> {
  return new Promise<void>((resolve) => setTimeout(resolve, ms));
}
