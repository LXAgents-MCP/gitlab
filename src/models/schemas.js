/**
 * Schema fragments and annotations shared by the tools, so a repository, a page or a hint is
 * described once and every tool says it the same way.
 */
import { z } from "zod";

/** The `repo` argument every tool takes except `repo_create` and `repo_search`. A repository is a project here. */
export const repoArg = z
  .string()
  .describe("Project: its full path, such as 'lxagents-mcp/gitlab' or 'group/subgroup/project', or its numeric id.");

/** Paging arguments for a list. */
export const pagingArgs = {
  page: z.number().int().min(1).optional().describe("Page number, from 1. Default 1."),
  per_page: z.number().int().min(1).max(100).optional().describe("Results per page, 1 to 100. Default 30."),
};

/** Paging fields of a list result. */
export const pagingOut = {
  page: z.number(),
  per_page: z.number(),
  has_more: z.boolean(),
};

/*
 * Annotations tell the client what a tool will do, so it can decide whether to ask first.
 * They are honest or they are worse than none: a read that claims to be read-only and writes
 * is trusted exactly because it said so. `openWorldHint` is true everywhere - every tool
 * talks to GitLab.
 */

/** Reads and changes nothing. */
export const READ = { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true };

/** Creates something new; repeating the call creates it again or fails. */
export const WRITE = { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: true };

/** Changes something that exists; repeating the call with the same arguments changes nothing more. */
export const UPDATE = { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true };

/** Deletes, overwrites or merges, which cannot be taken back from here. */
export const DESTROY = { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true };
