/**
 * People, by username. GitLab assigns by numeric user id, and the tools take the username a person
 * would write, so a name is looked up before anything is changed.
 */
import { gitlab } from "./api.js";
import { Refused } from "./errors.js";

/**
 * A username as people write it, with or without the `@`.
 *
 * @param {string} name
 * @returns {string}
 */
export function bareUsername(name) {
  return name.replace(/^@/, "");
}

/**
 * Find the users behind some usernames, and refuse before any write if one does not exist.
 *
 * A name GitLab does not know is a typo far more often than a wish, and sending the rest anyway
 * would assign half of what was asked and say nothing about it.
 *
 * @param {string[]} names Usernames, already trimmed and without duplicates.
 * @param {string} what What the lookup is for, completing "... to <what>" in a refusal.
 * @returns {Promise<{ id: number, username: string }[]>} In the order asked.
 */
export async function findUsers(names, what) {
  const found = [];
  const unknown = [];

  for (const requested of names) {
    const username = bareUsername(requested);
    const { body } = await gitlab("GET", "/users", { query: { username }, what: `look up the user '${username}' to ${what}` });
    const user = (Array.isArray(body) ? body : []).find(
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
