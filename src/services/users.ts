/**
 * People, by username. GitLab assigns by numeric user id, and the tools take the username a person
 * would write, so a name is looked up before anything is changed.
 */
import { gitlab } from "./api.js";
import { Refused } from "../utils/errors.js";
import type { ApiBody } from "../models/api-body.js";

/** A user GitLab knows, by id and username. */
export interface FoundUser {
  id: number;
  username: string;
}

/**
 * A username as people write it, with or without the `@`.
 */
export function bareUsername(name: string): string {
  return name.replace(/^@/, "");
}

/**
 * Find the users behind some usernames, and refuse before any write if one does not exist.
 *
 * A name GitLab does not know is a typo far more often than a wish, and sending the rest anyway
 * would assign half of what was asked and say nothing about it.
 *
 * @param names Usernames, already trimmed and without duplicates.
 * @param what What the lookup is for, completing "... to <what>" in a refusal.
 * @returns In the order asked.
 */
export async function findUsers(names: string[], what: string): Promise<FoundUser[]> {
  const found: FoundUser[] = [];
  const unknown: string[] = [];

  for (const requested of names) {
    const username = bareUsername(requested);
    const { body } = await gitlab("GET", "/users", { query: { username }, what: `look up the user '${username}' to ${what}` });
    const user = (Array.isArray(body) ? (body as ApiBody[]) : []).find(
      (candidate) => typeof candidate?.username === "string" && candidate.username.toLowerCase() === username.toLowerCase(),
    );
    if (user === undefined || !Number.isInteger(user.id)) unknown.push(username);
    else found.push({ id: user.id, username: user.username });
  }

  if (unknown.length > 0) {
    throw new Refused(
      `GitLab has no user named ${unknown.map((name) => `'${name}'`).join(", ")}, so nothing was changed. ` +
        "Write the username as it appears after the @ on the profile page.",
    );
  }
  return found;
}
