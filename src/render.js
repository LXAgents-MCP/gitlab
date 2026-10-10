/**
 * How a payload becomes the text the model reads.
 *
 * Every tool returns `{ summary, ...fields }`. The summary is the answer in a sentence; the
 * fields are what a client reads programmatically, and the text shows them too, because the
 * text is what the model actually sees:
 *
 * - a scalar field is a bullet, `- key: value`;
 * - a list of scalars is one bullet, joined with commas;
 * - a list of rows (`items`, `files`, `commits`, `comments`) is a heading and one bullet per row,
 *   and a long or multi-line field of a row (a comment's body) is a block indented under its bullet;
 * - `text` is a block - a file, a log, a diff - fenced so it cannot be mistaken for prose;
 * - any other string with a line break, or too long for a bullet, is a block under its own name,
 *   so a commit message or an issue body keeps its lines;
 * - a nested object is left to the structured content.
 */
import { MAX_OUTPUT } from "./config.js";

const CELL_MAX = 200;

const isScalar = (value) => ["string", "number", "boolean"].includes(typeof value);

/** A string that would not fit a bullet: it has a line break, or it is longer than a cell. */
const isBlockString = (value) => typeof value === "string" && (value.includes("\n") || value.length > CELL_MAX);

/** One value of a row, on one line. */
function cell(value) {
  if (value === undefined || value === null) return "";
  if (Array.isArray(value)) return value.every(isScalar) ? value.join(", ") : "";
  if (!isScalar(value)) return "";
  const line = String(value).replace(/\s*\n\s*/g, " ");
  return line.length > CELL_MAX ? `${line.slice(0, CELL_MAX)}…` : line;
}

/**
 * A row as one line: `key: value · key: value`, scalars only.
 *
 * @param {unknown} row
 * @returns {string}
 */
export function describe(row) {
  if (isScalar(row)) return cell(row);
  if (!row || typeof row !== "object") return "";
  return Object.entries(row)
    .filter(([, value]) => !isBlockString(value))
    .map(([key, value]) => [key, cell(value)])
    .filter(([, text]) => text !== "")
    .map(([key, text]) => `${key}: ${text}`)
    .join(" · ");
}

/** One row of a list: a bullet for its short fields, then each long field as an indented block. */
function renderRow(row) {
  const lines = [`- ${describe(row)}`];
  if (row && typeof row === "object") {
    for (const [key, value] of Object.entries(row)) {
      if (!isBlockString(value)) continue;
      lines.push(...`${key}:\n${fence(value)}`.split("\n").map((line) => `  ${line}`));
    }
  }
  return lines.join("\n");
}

/**
 * Fence a block with more backticks than any run inside it, so the content cannot close the fence.
 *
 * @param {string} text
 * @returns {string}
 */
export function fence(text) {
  const longest = Math.max(0, ...(text.match(/`+/g) ?? []).map((run) => run.length));
  const ticks = "`".repeat(Math.max(3, longest + 1));
  return `${ticks}text\n${text.replace(/\n$/, "")}\n${ticks}`;
}

/**
 * @param {string} summary
 * @param {Record<string, unknown>} fields
 * @returns {string}
 */
export function renderPayload(summary, fields) {
  const parts = [`**${summary}**`];
  const bullets = [];
  const lists = [];
  const blocks = [];

  for (const [key, value] of Object.entries(fields)) {
    if (value === undefined || value === null) continue;
    if (key === "text" && typeof value === "string") {
      if (value !== "") blocks.push({ text: value });
    } else if (isBlockString(value)) {
      blocks.push({ label: key, text: value });
    } else if (Array.isArray(value)) {
      if (value.length === 0) continue;
      if (value.every(isScalar)) bullets.push(`- ${key}: ${value.join(", ")}`);
      else lists.push([key, value]);
    } else if (isScalar(value)) {
      bullets.push(`- ${key}: ${cell(value)}`);
    }
  }

  if (bullets.length > 0) parts.push("", bullets.join("\n"));
  for (const [key, rows] of lists) {
    parts.push("", `**${key}**`, rows.map(renderRow).join("\n"));
  }
  for (const { label, text } of blocks) {
    parts.push("");
    if (label) parts.push(`**${label}**`);
    parts.push(fence(text));
  }

  const text = parts.join("\n");
  return text.length > MAX_OUTPUT ? `${text.slice(0, MAX_OUTPUT)}\n... (truncated)` : text;
}
