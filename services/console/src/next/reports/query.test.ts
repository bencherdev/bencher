import { expect, test } from "vitest";
import type { Api } from "../api";
import { type ReportsBatch, batchKey, batchQuery } from "./query";
import { DEFAULT_SEARCH } from "./search";

const api = {} as Api;
const BATCH = { page: 1, perPage: 20 };
const previous: ReportsBatch = { reports: [], total: 3, batch: BATCH };
const queryOf = (slug: string) => ({
	queryKey: batchKey(slug, { ...DEFAULT_SEARCH, alerts: true }, BATCH),
});

// Kills a project's first batch drawn from another project's rows while it loads.
test("a first batch holds the last search's rows only for the same project", () => {
	const { placeholderData } = batchQuery(
		api,
		"hashbrown",
		DEFAULT_SEARCH,
		BATCH,
	);

	expect(placeholderData(previous, queryOf("hashbrown"))).toBe(previous);
	expect(placeholderData(previous, queryOf("croissant"))).toBeUndefined();
});
