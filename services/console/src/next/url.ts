/** The longest request URL every edge passes on: Cloudflare's, below the API's own. */
export const MAX_URL_LENGTH = 16_384;

/** What a request past that length reads as, sent or not. */
export const URI_TOO_LONG = 414;
