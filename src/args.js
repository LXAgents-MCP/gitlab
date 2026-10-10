/**
 * Argument checks. Every tool validates its own arguments before it reads the token or sends
 * anything, so a bad call costs no request and the refusal names the argument.
 */
import { Refused } from "./errors.js";

/**
 * An optional string argument: trimmed, and `undefined` when absent or blank.
 *
 * @param {unknown} value
 * @param {string} field
 * @returns {string | undefined}
 */
export function optionalText(value, field) {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string") throw new Refused(`\`${field}\` must be a string, got ${typeof value}.`);
  const trimmed = value.trim();
  return trimmed === "" ? undefined : trimmed;
}

/**
 * A required string argument: trimmed, and never blank.
 *
 * @param {unknown} value
 * @param {string} field
 * @returns {string}
 */
export function requiredText(value, field) {
  const text = optionalText(value, field);
  if (text === undefined) throw new Refused(`\`${field}\` is required and must be a non-empty string.`);
  return text;
}

/**
 * Text kept exactly as given - a file's content, a comment body - and only checked to be a string.
 *
 * @param {unknown} value
 * @param {string} field
 * @param {{ required?: boolean, allowEmpty?: boolean }} [options] `required` refuses a missing value; a
 *   required value may not be blank unless `allowEmpty`.
 * @returns {string | undefined}
 */
export function rawText(value, field, { required = false, allowEmpty = false } = {}) {
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
 *
 * @param {unknown} value
 * @param {string} field
 * @param {boolean} fallback
 * @returns {boolean}
 */
export function optionalFlag(value, field, fallback) {
  if (value === undefined || value === null) return fallback;
  if (typeof value !== "boolean") throw new Refused(`\`${field}\` must be true or false, got ${typeof value}.`);
  return value;
}

/**
 * An optional boolean with no default, so "not given" and "false" stay different.
 *
 * @param {unknown} value
 * @param {string} field
 * @returns {boolean | undefined}
 */
export function maybeFlag(value, field) {
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "boolean") throw new Refused(`\`${field}\` must be true or false, got ${typeof value}.`);
  return value;
}

/**
 * An optional whole number inside a range.
 *
 * @param {unknown} value
 * @param {string} field
 * @param {{ min?: number, max?: number }} [range]
 * @returns {number | undefined}
 */
export function optionalInt(value, field, { min = 1, max = Number.MAX_SAFE_INTEGER } = {}) {
  if (value === undefined || value === null) return undefined;
  if (!Number.isInteger(value)) throw new Refused(`\`${field}\` must be a whole number, got ${JSON.stringify(value)}.`);
  if (value < min || value > max) {
    throw new Refused(`\`${field}\` must be between ${min} and ${max}, got ${value}.`);
  }
  return value;
}

/**
 * A required whole number inside a range.
 *
 * @param {unknown} value
 * @param {string} field
 * @param {{ min?: number, max?: number }} [range]
 * @returns {number}
 */
export function requiredInt(value, field, range) {
  const number = optionalInt(value, field, range);
  if (number === undefined) throw new Refused(`\`${field}\` is required and must be a whole number.`);
  return number;
}

/**
 * An optional date or timestamp, in ISO 8601 form such as `2026-10-01` or `2026-10-01T12:00:00Z`.
 *
 * @param {unknown} value
 * @param {string} field
 * @returns {string | undefined}
 */
export function optionalDate(value, field) {
  const text = optionalText(value, field);
  if (text === undefined) return undefined;
  if (!/^\d{4}-\d{2}-\d{2}/.test(text) || Number.isNaN(Date.parse(text))) {
    throw new Refused(`\`${field}\` must be an ISO 8601 date such as 2026-10-01 or 2026-10-01T12:00:00Z, got '${text}'.`);
  }
  return text;
}

/**
 * One of a fixed set of words, with the value to use when it is absent.
 *
 * @template {string} T
 * @param {unknown} value
 * @param {string} field
 * @param {readonly T[]} allowed
 * @param {T | undefined} [fallback]
 * @returns {T | undefined}
 */
export function oneOf(value, field, allowed, fallback = undefined) {
  const text = optionalText(value, field);
  if (text === undefined) return fallback;
  if (!allowed.includes(/** @type {T} */ (text))) {
    throw new Refused(`\`${field}\` must be one of ${allowed.join(", ")}, got '${text}'.`);
  }
  return /** @type {T} */ (text);
}

/**
 * A list of non-blank strings, trimmed and without duplicates.
 *
 * @param {unknown} value
 * @param {string} field
 * @param {{ required?: boolean, max?: number }} [options]
 * @returns {string[] | undefined}
 */
export function textList(value, field, { required = false, max = 100 } = {}) {
  if (value === undefined || value === null) {
    if (required) throw new Refused(`\`${field}\` is required and must list at least one value.`);
    return undefined;
  }
  if (!Array.isArray(value)) throw new Refused(`\`${field}\` must be a list of strings.`);
  const seen = new Set();
  for (const entry of value) {
    if (typeof entry !== "string") throw new Refused(`\`${field}\` must be a list of strings.`);
    const text = entry.trim();
    if (text !== "") seen.add(text);
  }
  if (seen.size > max) throw new Refused(`\`${field}\` may hold at most ${max} values, got ${seen.size}.`);
  if (required && seen.size === 0) throw new Refused(`\`${field}\` is required and must list at least one value.`);
  return [...seen];
}
