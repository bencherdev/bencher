import { expect, test } from "vitest";
import { type ExploreQuery, blankQuery, decodeQuery } from "../query/query";
import { publicPlotPath, publicUrl } from "./query";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1), head: uuid(11) }],
	testbeds: [{ uuid: uuid(3) }],
	benchmarks: [uuid(5), uuid(6)],
	sets: [{ bits: 8 }],
	measures: [uuid(7)],
	metrics: ["value"],
	xAxis: "version",
	hide: ["a"],
	focus: "b",
};

// Kills a link to the console's own page, one to another project's plot, one
// that keeps the pin only the console reads, and one that drops the rest of
// the query with it.
test("publicUrl links to the public plot of the query, without its pin", () => {
	const url = new URL(
		publicUrl("https://bencher.dev", publicPlotPath("hashbrown"), {
			...QUERY,
			plot: uuid(40),
		}),
	);
	expect(url.origin).toBe("https://bencher.dev");
	expect(url.pathname).toBe("/perf/hashbrown");
	expect(url.searchParams.has("plot")).toBe(false);
	expect(decodeQuery(url.search)).toEqual(QUERY);
});
