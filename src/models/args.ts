/**
 * Argument checks. Every tool validates its own arguments before it reads the token or sends
 * anything, so a bad call costs no request and the refusal names the argument.
 */
import { Refused } from "../utils/errors.js";

/**
 * An optional string argument: trimmed, and `undefined` when absent or blank.
 */
export function optionalText(value: unknown, field: string): string | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string") throw new Refused(`\`${field}\` must be a string, got ${typeof value}.`);
  const trimmed = value.trim();
  return trimmed === "" ? undefined : trimmed;
}

/**
 * A required string argument: trimmed, and never blank.
 */
export function requiredText(value: unknown, field: string): string {
  const text = optionalText(value, field);
  if (text === undefined) throw new Refused(`\`${field}\` is required and must be a non-empty string.`);
  return text;
}

/**
 * Text kept exactly as given - a file's content, a comment body - and only checked to be a string.
 *
 * @param options `required` refuses a missing value; a
 *   required value may not be blank unless `allowEmpty`.
 */
export function rawText(
  value: unknown,
  field: string,
  options: { required: true; allowEmpty?: boolean },
): string;
export function rawText(
  value: unknown,
  field: string,
  options?: { required?: boolean; allowEmpty?: boolean },
): string | undefined;
export function rawText(
  value: unknown,
  field: string,
  { required = false, allowEmpty = false }: { required?: boolean; allowEmpty?: boolean } = {},
): string | undefined {
  if (value === undefined || value === null) {
    if (required) throw new Refused(`\`${field}\` is required.`);
    return undefined;
  }
  if (typeof value !== "string") throw new Refused(`\`${field}\` must be a string, got ${typeof value}.`);
  if (required && !allowEmpty && value.trim() === "") {
    throw new Refused(`\`${field}\` is required and must not be blank.`);
  }
  return value;
}

/**
 * An optional boolean argument, with the value to use when it is absent.
 */
export function optionalFlag(value: unknown, field: string, fallback: boolean): boolean {
  if (value === undefined || value === null) return fallback;
  if (typeof value !== "boolean") throw new Refused(`\`${field}\` must be true or false, got ${typeof value}.`);
  return value;
}

/**
 * An optional boolean with no default, so "not given" and "false" stay different.
 */
export function maybeFlag(value: unknown, field: string): boolean | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "boolean") throw new Refused(`\`${field}\` must be true or false, got ${typeof value}.`);
  return value;
}

/**
 * An optional whole number inside a range.
 */
/** The bounds of a whole-number argument. */
export interface IntRange {
  min?: number;
  max?: number;
}

export function optionalInt(
  value: unknown,
  field: string,
  { min = 1, max = Number.MAX_SAFE_INTEGER }: IntRange = {},
): number | undefined {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "number" || !Number.isInteger(value)) throw new Refused(`\`${field}\` must be a whole number, got ${JSON.stringify(value)}.`);
  if (value < min || value > max) {
    throw new Refused(`\`${field}\` must be between ${min} and ${max}, got ${value}.`);
  }
  return value;
}

/**
 * A required whole number inside a range.
 */
export function requiredInt(value: unknown, field: string, range?: IntRange): number {
  const number = optionalInt(value, field, range);
  if (number === undefined) throw new Refused(`\`${field}\` is required and must be a whole number.`);
  return number;
}

/**
 * An optional date or timestamp, in ISO 8601 form such as `2026-10-01` or `2026-10-01T12:00:00Z`.
 */
export function optionalDate(value: unknown, field: string): string | undefined {
  const text = optionalText(value, field);
  if (text === undefined) return undefined;
  if (!/^\d{4}-\d{2}-\d{2}/.test(text) || Number.isNaN(Date.parse(text))) {
    throw new Refused(`\`${field}\` must be an ISO 8601 date such as 2026-10-01 or 2026-10-01T12:00:00Z, got '${text}'.`);
  }
  return text;
}

/**
 * One of a fixed set of words, with the value to use when it is absent.
 */
export function oneOf<T extends string>(
  value: unknown,
  field: string,
  allowed: readonly T[],
  fallback: T | undefined = undefined,
): T | undefined {
  const text = optionalText(value, field);
  if (text === undefined) return fallback;
  if (!allowed.includes(text as T)) {
    throw new Refused(`\`${field}\` must be one of ${allowed.join(", ")}, got '${text}'.`);
  }
  return text as T;
}

/**
 * A list of non-blank strings, trimmed and without duplicates.
 */
/** The options of a list argument. */
export interface ListOptions {
  required?: boolean;
  max?: number;
}

export function textList(
  value: unknown,
  field: string,
  { required = false, max = 100 }: ListOptions = {},
): string[] | undefined {
  if (value === undefined || value === null) {
    if (required) throw new Refused(`\`${field}\` is required and must list at least one value.`);
    return undefined;
  }
  if (!Array.isArray(value)) throw new Refused(`\`${field}\` must be a list of strings.`);
  const seen = new Set<string>();
  for (const entry of value) {
    if (typeof entry !== "string") throw new Refused(`\`${field}\` must be a list of strings.`);
    const text = entry.trim();
    if (text !== "") seen.add(text);
  }
  if (seen.size > max) throw new Refused(`\`${field}\` may hold at most ${max} values, got ${seen.size}.`);
  if (required && seen.size === 0) throw new Refused(`\`${field}\` is required and must list at least one value.`);
  return [...seen];
}

/**
 * A list of label names. GitLab takes labels as one comma-separated string, so a name with a comma
 * in it cannot be sent and is refused here instead of being split into two labels.
 */
export function labelList(value: unknown, field: string, options?: ListOptions): string[] | undefined {
  const labels = textList(value, field, options);
  const bad = (labels ?? []).find((label) => label.includes(","));
  if (bad !== undefined) {
    throw new Refused(`\`${field}\` may not contain a comma, because GitLab separates label names with one: '${bad}'.`);
  }
  return labels;
}
