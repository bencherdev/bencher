import { describe, expect, test } from "vitest";
import {
	DEFAULT_SEARCH,
	apiParams,
	decodeSearch,
	directionText,
	encodeSearch,
	flipDirection,
	pickSort,
} from "./search";

const decode = (query: string) => decodeSearch(new URLSearchParams(query));

describe("decodeSearch", () => {
	// Kills a blank link that is not the active rows by name, A to Z.
	test("a blank link is the active rows by name, A to Z", () => {
		expect(decode("")).toEqual({
			archived: false,
			sort: "name",
			direction: "asc",
			search: "",
		});
		expect(DEFAULT_SEARCH).toEqual(decode(""));
	});

	// Kills a sort read without its own default direction, and a hand-edited
	// value that breaks the page.
	test("each sort starts in its own direction, and unknown values fall back", () => {
		expect(decode("sort=last_used")).toMatchObject({
			sort: "last_used",
			direction: "desc",
		});
		expect(decode("sort=created&direction=asc")).toMatchObject({
			sort: "created",
			direction: "asc",
		});
		expect(decode("sort=size&direction=up&archived=yes")).toEqual(
			DEFAULT_SEARCH,
		);
		expect(decode("archived=true&search=%20simd%20")).toMatchObject({
			archived: true,
			search: "simd",
		});
	});
});

describe("encodeSearch", () => {
	// Kills a link that carries its defaults, or loses a choice on the way back.
	test("writes only what differs from the defaults, and reads back the same", () => {
		expect(encodeSearch(DEFAULT_SEARCH)).toBe("");
		expect(
			encodeSearch({ ...DEFAULT_SEARCH, sort: "last_used", direction: "desc" }),
		).toBe("sort=last_used");
		for (const search of [
			{ archived: true, sort: "name", direction: "desc", search: "pr" },
			{ archived: false, sort: "last_used", direction: "asc", search: "" },
			{ archived: false, sort: "created", direction: "desc", search: "a b" },
		] as const) {
			expect(decode(encodeSearch(search))).toEqual(search);
		}
	});
});

describe("sorting", () => {
	// Kills a picked sort that keeps the last one's direction.
	test("picking a sort starts it in its own direction", () => {
		const byNameDown = { ...DEFAULT_SEARCH, direction: "desc" as const };
		expect(pickSort(byNameDown, "last_used")).toMatchObject({
			sort: "last_used",
			direction: "desc",
		});
		expect(
			pickSort(
				{ ...DEFAULT_SEARCH, sort: "created", direction: "asc" },
				"name",
			),
		).toMatchObject({
			sort: "name",
			direction: "asc",
		});
	});

	// Kills a flip that does nothing or leaves the sort.
	test("the direction flips both ways", () => {
		const newest = pickSort(DEFAULT_SEARCH, "last_used");
		expect(flipDirection(newest)).toMatchObject({
			sort: "last_used",
			direction: "asc",
		});
		expect(flipDirection(flipDirection(newest))).toEqual(newest);
	});

	// Kills a direction named for the wrong kind of sort.
	test("names reads alphabetically, times by age", () => {
		expect(directionText("name", "asc")).toBe("A to Z");
		expect(directionText("name", "desc")).toBe("Z to A");
		expect(directionText("created", "desc")).toBe("Newest first");
		expect(directionText("last_used", "asc")).toBe("Oldest first");
	});
});

// Kills a batch the API pages differently, a sort it never hears, and an
// archived list asked for as the active one.
test("apiParams asks for one batch as the search says", () => {
	expect(apiParams(DEFAULT_SEARCH, { offset: 0, perPage: 26 })).toBe(
		"sort=name&direction=asc&offset=0&per_page=26",
	);
	expect(
		apiParams(
			{ archived: true, sort: "last_used", direction: "asc", search: "pr 4" },
			{ offset: 77, perPage: 40 },
		),
	).toBe(
		"archived=true&sort=last_used&direction=asc&search=pr+4&offset=77&per_page=40",
	);
});
