/** The paging arguments every list tool takes, and the paging fields it returns. */
import { DEFAULT_PER_PAGE, MAX_PER_PAGE } from "../config/config.js";
import { optionalInt } from "./args.js";

/**
 * Read `page` and `per_page` from a tool's arguments.
 */
/** The page a list tool asks GitLab for. */
export interface PageQuery {
  page: number;
  per_page: number;
}

/** The paging fields of a list result. */
export interface Paging extends PageQuery {
  has_more: boolean;
}

export function pageQuery(args: { page?: unknown; per_page?: unknown }): PageQuery {
  return {
    page: optionalInt(args.page, "page", { min: 1 }) ?? 1,
    per_page: optionalInt(args.per_page, "per_page", { min: 1, max: MAX_PER_PAGE }) ?? DEFAULT_PER_PAGE,
  };
}

/**
 * The paging fields of a list result: the page asked for, and whether GitLab has another.
 *
 * @param headers The response headers, lowercased.
 */
export function paging(query: PageQuery, headers: Record<string, string>): Paging {
  // Most answers carry `x-next-page`; the commits API, which sends no totals, is read by its link header too.
  const next = (headers["x-next-page"] ?? "") !== "" || /rel="next"/.test(headers.link ?? "");
  return { page: query.page, per_page: query.per_page, has_more: next };
}
