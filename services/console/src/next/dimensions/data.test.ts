import { QueryClient } from "@tanstack/solid-query";
import { describe, expect, test } from "vitest";
import type { JsonConsoleBranchRow, JsonVariant } from "../../types/bencher";
import { DIMENSIONS } from "../paths";
import { decodeQuery } from "../query/query";
import { fakeApi } from "../reports/testing";
import {
	type Row,
	type RowBatch,
	type Variants,
	afterArchive,
	explorePath,
	listQuery,
	markArchived,
	markVariantArchived,
	rowQuery,
	thresholdsQuery,
	withKept,
} from "./data";
import { DEFAULT_SEARCH } from "./search";

const row = (name: string, fields: Partial<JsonConsoleBranchRow> = {}) =>
	({
		uuid: `uuid-${name}`,
		name,
		slug: name,
		created: 1,
		thresholds: 1,
		held_thresholds: 0,
		...fields,
	}) as JsonConsoleBranchRow;

const batch = (rows: JsonConsoleBranchRow[], page = 1): RowBatch => ({
	rows,
	total: 3,
	active: 3,
	archived: 1,
	batch: { ordinal: page, perPage: 2 },
});

const client = () =>
	new QueryClient({ defaultOptions: { queries: { retry: false } } });

describe("markArchived", () => {
	// Kills a change shown in one batch only, totals left as they were, and a
	// change that moves other rows with it.
	test("moves one row and the totals in every cached batch", () => {
		const cache = client();
		const first = ["console", "branches", "hashbrown", "list", "", 2, 1];
		const second = ["console", "branches", "hashbrown", "list", "", 2, 2];
		const searched = [
			"console",
			"branches",
			"hashbrown",
			"list",
			"search=e",
			2,
			1,
		];
		const testbeds = ["console", "testbeds", "hashbrown", "list", "", 2, 1];
		cache.setQueryData(first, batch([row("devel"), row("feature-simd")]));
		cache.setQueryData(second, batch([row("main")], 2));
		cache.setQueryData(searched, batch([row("feature-simd")]));
		cache.setQueryData(testbeds, batch([row("devel")]));

		markArchived(cache, "hashbrown", "branches", "uuid-feature-simd", 5);
		const archivedOf = (key: unknown[]) =>
			cache.getQueryData<RowBatch>(key)?.rows.map((each) => each.archived);
		expect(archivedOf(first)).toEqual([undefined, 5]);
		expect(archivedOf(searched)).toEqual([5]);
		expect(archivedOf(testbeds)).toEqual([undefined]);
		for (const key of [first, second, searched]) {
			expect(cache.getQueryData<RowBatch>(key)).toMatchObject({
				active: 2,
				archived: 2,
				total: 3,
			});
		}
		expect(cache.getQueryData<RowBatch>(testbeds)).toMatchObject({
			active: 3,
			archived: 1,
		});

		markArchived(cache, "hashbrown", "branches", "uuid-devel", 6);
		markArchived(
			cache,
			"hashbrown",
			"branches",
			"uuid-feature-simd",
			undefined,
		);
		expect(archivedOf(first)).toEqual([6, undefined]);
		expect(cache.getQueryData<RowBatch>(second)).toMatchObject({
			active: 2,
			archived: 2,
		});
	});

	// Kills totals that move whatever the row showed, which a refusal after an
	// Undo would leave at "Active 4" over three rows.
	test("the totals move only when the row's shown state changes", () => {
		const cache = client();
		const key = ["console", "branches", "hashbrown", "list", "", 2, 1];
		cache.setQueryData(key, batch([row("devel"), row("main")]));
		const totals = () => {
			const { active, archived } = cache.getQueryData<RowBatch>(key) ?? {};
			return { active, archived };
		};

		markArchived(cache, "hashbrown", "branches", "uuid-devel", 5);
		markArchived(cache, "hashbrown", "branches", "uuid-devel", 6);
		expect(totals()).toEqual({ active: 2, archived: 2 });
		expect(cache.getQueryData<RowBatch>(key)?.rows[0]?.archived).toBe(6);
		markArchived(cache, "hashbrown", "branches", "uuid-devel", undefined);
		markArchived(cache, "hashbrown", "branches", "uuid-devel", undefined);
		expect(totals()).toEqual({ active: 3, archived: 1 });
	});

	// Kills a page whose row the cache does not hold moving the totals twice,
	// or not at all.
	test("a row the cache does not hold moves the totals from the state it was shown in", () => {
		const cache = client();
		const key = ["console", "branches", "hashbrown", "list", "", 2, 1];
		cache.setQueryData(key, batch([row("devel")]));
		const totals = () => {
			const { active, archived } = cache.getQueryData<RowBatch>(key) ?? {};
			return { active, archived };
		};

		markArchived(cache, "hashbrown", "branches", "uuid-main", 5, false);
		expect(totals()).toEqual({ active: 2, archived: 2 });
		markArchived(cache, "hashbrown", "branches", "uuid-main", 6, true);
		expect(totals()).toEqual({ active: 2, archived: 2 });
		markArchived(cache, "hashbrown", "branches", "uuid-main", undefined, true);
		expect(totals()).toEqual({ active: 3, archived: 1 });
	});
});

// Kills an archive that leaves the shell's alert count and other pages stale,
// and one that refetches the list on screen, which would drop the dimmed row.
test("afterArchive marks the rest of the project stale and refetches only what is shown elsewhere", async () => {
	const cache = client();
	const list = ["console", "branches", "hashbrown", "list", "", 2, 1];
	const shell = ["console", "project", "hashbrown"];
	const other = ["console", "project", "other"];
	const thresholds = ["console", "thresholds", "hashbrown", "on"];
	for (const key of [list, shell, other, thresholds]) {
		cache.setQueryData(key, {});
	}
	afterArchive(cache, "hashbrown", "branches");
	const stale = (key: unknown[]) =>
		cache.getQueryCache().find({ queryKey: key, exact: true })?.state
			.isInvalidated;
	expect(stale(shell)).toBe(true);
	expect(stale(thresholds)).toBe(true);
	expect(stale(list)).toBe(true);
	expect(stale(other)).toBe(false);
});

// Kills an Explore link that opens a dimension in another dimension's box.
test.each(DIMENSIONS)(
	"explorePath opens Explore with one of %s in its own box",
	(dimension) => {
		const uuid = "00000000-0000-4000-8000-000000000001";
		const [path, search] = explorePath("hashbrown", dimension, uuid).split("?");
		expect(path).toBe("/next/console/projects/hashbrown/explore");
		const query = decodeQuery(search ?? "");
		expect({
			branches: query.branches.map((entry) => entry.uuid),
			testbeds: query.testbeds.map((entry) => entry.uuid),
			benchmarks: query.benchmarks,
			measures: query.measures,
		}).toEqual({
			branches: [],
			testbeds: [],
			benchmarks: [],
			measures: [],
			[dimension]: [uuid],
		});
	},
);

describe("queries", () => {
	// Kills a row taken from a looser match than its own slug.
	test("rowQuery picks the row its path names out of what the search matches", async () => {
		const { api, requests } = fakeApi(() => ({
			data: { branches: [row("main-2"), row("main"), row("domain")] },
		}));
		const query = rowQuery(api, "hashbrown", "branches", "main", true);
		const found = await query.queryFn({ signal: new AbortController().signal });
		expect(found?.slug).toBe("main");
		expect(requests[0]?.searchParams.get("search")).toBe("main");
		expect(requests[0]?.searchParams.get("archived")).toBe("true");
		const none = await rowQuery(
			api,
			"hashbrown",
			"branches",
			"nope",
			false,
		).queryFn({
			signal: new AbortController().signal,
		});
		expect(none).toBeNull();
	});

	// Kills thresholds asked for under another dimension's name.
	test.each([
		["branches", "branch"],
		["testbeds", "testbed"],
		["measures", "measure"],
	] as const)("thresholdsQuery filters %s by %s", async (dimension, param) => {
		const { api, requests } = fakeApi(() => ({ data: [], total: 3 }));
		const answer = await thresholdsQuery(
			api,
			"hashbrown",
			dimension,
			"x",
			false,
		).queryFn({
			signal: new AbortController().signal,
		});
		expect(answer.total).toBe(3);
		expect(requests[0]?.pathname).toBe("/v0/projects/hashbrown/thresholds");
		expect(requests[0]?.searchParams.get(param)).toBe("x");
		expect(requests[0]?.searchParams.get("archived")).toBe("false");
	});

	// Kills a list read from another dimension's rows, and a batch asked for
	// by page, which skips the rows archived on screen.
	test("listQuery reads the rows under its dimension's name, from the offset it is given", async () => {
		const { api, requests } = fakeApi(() => ({
			data: { total: 1, active: 4, archived: 2, testbeds: [row("linux")] },
		}));
		const answer = await listQuery(
			api,
			"hashbrown",
			"testbeds",
			DEFAULT_SEARCH,
			{ ordinal: 2, perPage: 20 },
			async () => 17,
		).queryFn({ signal: new AbortController().signal });
		expect(answer).toMatchObject({ total: 1, active: 4, archived: 2 });
		expect(answer.rows.map(({ name }) => name)).toEqual(["linux"]);
		expect(requests[0]?.pathname).toBe(
			"/v0/projects/hashbrown/console/testbeds",
		);
		expect(requests[0]?.searchParams.get("offset")).toBe("17");
		expect(requests[0]?.searchParams.get("per_page")).toBe("20");
		expect(requests[0]?.searchParams.has("page")).toBe(false);
	});
});

// Kills a variant shown archived in its active list without the totals
// moving, and totals that move whatever the variant showed.
test("markVariantArchived moves a variant and both totals only when its shown state changes", () => {
	const cache = client();
	const active = [
		"console",
		"benchmarks",
		"hashbrown",
		"variants",
		"blake3",
		false,
	];
	const archived = [
		"console",
		"benchmarks",
		"hashbrown",
		"variants",
		"blake3",
		true,
	];
	const variant = {
		uuid: "v1",
		parameters: { threads: 4 },
	} as unknown as JsonVariant;
	cache.setQueryData<Variants>(active, { variants: [variant], total: 8 });
	cache.setQueryData<Variants>(archived, { variants: [], total: 0 });
	const totals = () => [
		cache.getQueryData<Variants>(active)?.total,
		cache.getQueryData<Variants>(archived)?.total,
	];

	markVariantArchived(
		cache,
		"hashbrown",
		"blake3",
		"v1",
		"2026-09-14T00:00:00Z",
	);
	markVariantArchived(
		cache,
		"hashbrown",
		"blake3",
		"v1",
		"2026-09-15T00:00:00Z",
	);
	expect(cache.getQueryData<Variants>(active)?.variants[0]?.archived).toBe(
		"2026-09-15T00:00:00Z",
	);
	expect(totals()).toEqual([7, 1]);
	markVariantArchived(cache, "hashbrown", "blake3", "v1", undefined);
	markVariantArchived(cache, "hashbrown", "blake3", "v1", undefined);
	expect(
		cache.getQueryData<Variants>(active)?.variants[0]?.archived,
	).toBeUndefined();
	expect(totals()).toEqual([8, 0]);
});

describe("withKept", () => {
	const rows = ["a", "b", "c", "d"].map((name) => row(name));
	const at = (name: string) => rows.find((each) => each.name === name) as Row;
	const kept = (name: string, after: string | undefined) => ({
		row: { ...at(name), archived: 5 },
		after: after === undefined ? undefined : `uuid-${after}`,
	});
	const without = (...gone: string[]) =>
		rows.filter(({ name }) => !gone.includes(name));
	const names = (drawn: readonly Row[]) => drawn.map(({ name }) => name);

	// Kills a kept row the list left out dropped, put at the end, or put before
	// the row it followed.
	test("a row the list no longer holds comes back after the row it followed", () => {
		expect(names(withKept(without("c"), [kept("c", "b")]))).toEqual([
			"a",
			"b",
			"c",
			"d",
		]);
	});

	// Kills a first row, which follows none, dropped or moved to the end, and
	// rows kept one after another drawn out of order.
	test("rows kept one after another from the top keep their order", () => {
		expect(
			names(
				withKept(without("a", "b"), [kept("b", "a"), kept("a", undefined)]),
			),
		).toEqual(["a", "b", "c", "d"]);
	});

	// Kills a kept row the list still holds drawn as the API answered rather
	// than as the page last set it, or drawn twice.
	test("a row the list still holds is drawn once, as the page last set it", () => {
		const drawn = withKept(rows, [kept("b", "a")]);
		expect(names(drawn)).toEqual(["a", "b", "c", "d"]);
		expect(drawn[1]?.archived).toBe(5);
	});
});
