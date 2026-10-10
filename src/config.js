/**
 * Every knob this server has, in one place.
 *
 * There are no command-line flags and no configuration file. The only input from outside the
 * tool arguments is the token, read from the environment at call time (`requireToken` in
 * `api.js`), so a server without a token still starts and says what is missing when asked.
 */

/** The environment variable that holds the token. Never an argument, so it never appears in a prompt. */
export const TOKEN_ENV = "LXAGENTS_MCP_GITLAB_API_KEY";

/** The REST API. gitlab.com only; self-managed instances are not supported. */
export const API_BASE = "https://gitlab.com/api/v4";

/** Milliseconds. How long a call to GitLab may take before the tool gives up on it. */
export const API_TIMEOUT_MS = 30_000;

/** Bytes. The most of one answer the client will read; beyond it the call is refused. */
export const MAX_RESPONSE_BYTES = 20_000_000;

/** Characters. Past this the model reasons about a partial result as though it were complete. */
export const MAX_OUTPUT = 30_000;

/** Results per page when a call does not say, and the most GitLab allows. */
export const DEFAULT_PER_PAGE = 30;
export const MAX_PER_PAGE = 100;

/**
 * How `branch_merge` waits for GitLab to work out whether a new merge request can be merged: the
 * pause between reads, in milliseconds, and the most reads. GitLab does it in the background.
 */
export const MERGE_POLL_MS = 1_000;
export const MERGE_POLL_TRIES = 10;

/**
 * Characters of a file shown to the model. A file is read to be looked at, not ingested, and the
 * limit sits below `MAX_OUTPUT` so the summary and the fields still fit in the same result.
 */
export const MAX_FILE_CHARS = 25_000;

/** Bytes of a file `file_read` will take, and of content one write may carry. Source files fit; blobs are not what this is for. */
export const MAX_READ_BYTES = 1_000_000;
export const MAX_WRITE_BYTES = 1_000_000;

/** Characters of a diff shown to the model, which leaves room in a result for the rest of it. */
export const MAX_DIFF_CHARS = 20_000;

/** Changed files and commits listed in a result. The totals are always reported. */
export const MAX_FILES_SHOWN = 100;
export const MAX_COMMITS_SHOWN = 50;
