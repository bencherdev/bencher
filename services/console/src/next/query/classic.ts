import { decodeQuery, encodeQuery } from "./query";

/** Explore's query string for a classic perf page's: Explore reads the classic names, and drops the rest. */
export const exploreSearchFromClassic = (search: string): string =>
	encodeQuery(decodeQuery(search));
