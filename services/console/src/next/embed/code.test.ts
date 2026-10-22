import { Window } from "happy-dom";
import { describe, expect, test } from "vitest";
import { type ExploreQuery, blankQuery, decodeQuery } from "../query/query";
import { embedPath } from "../public/query";
import { embedCode, embedHeight } from "./code";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1), head: uuid(11) }],
	testbeds: [{ uuid: uuid(3) }],
	benchmarks: [uuid(5), uuid(6)],
	sets: [{ label: `it's "8" <bits> & more` }],
	measures: [uuid(7)],
	metrics: ["value"],
	xAxis: "version",
	window: { seconds: 7 * 24 * 60 * 60 },
	hide: ["a"],
	focus: "b",
};

const parse = (html: string) =>
	new new Window().DOMParser().parseFromString(html, "text/html").body;

describe("embedCode", () => {
	// Kills a frame of another page or another query, one that keeps the pin
	// the embed cannot read, and a frame with no fixed height.
	test("frames the embed of this query, at full width and a fixed height", () => {
		const html = embedCode({
			origin: "https://bencher.dev",
			slug: "hashbrown",
			query: { ...QUERY, plot: uuid(40) },
			name: "Hashbrown",
		});
		const body = parse(html);
		expect(body.children).toHaveLength(1);
		const frame = body.querySelector("iframe");
		const src = new URL(frame?.getAttribute("src") ?? "");
		expect(src.origin).toBe("https://bencher.dev");
		expect(src.pathname).toBe(embedPath("hashbrown"));
		expect(decodeQuery(src.search)).toEqual(QUERY);
		expect(frame?.getAttribute("title")).toBe("Hashbrown");
		expect(frame?.getAttribute("width")).toBe("100%");
		expect(frame?.getAttribute("height")).toBe(String(embedHeight(QUERY)));
	});

	// Kills a snippet of one height whatever the query draws, which cuts off
	// the panels of a query of more measures than the one it was sized for.
	test("sizes the frame from the query's measures and layout", () => {
		const height = (query: ExploreQuery) =>
			parse(
				embedCode({
					origin: "https://bencher.dev",
					slug: "hashbrown",
					query,
					name: "Hashbrown",
				}),
			)
				.querySelector("iframe")
				?.getAttribute("height");
		const measures = (count: number, layout: ExploreQuery["layout"] = "dual") =>
			height({
				...QUERY,
				measures: Array.from({ length: count }, (_, n) => uuid(70 + n)),
				layout,
			});
		const heights = [
			measures(1),
			measures(2),
			measures(2, "stacked"),
			measures(3),
			measures(8),
		].map(Number);
		expect(heights).toEqual([...heights].sort((a, b) => a - b));
		expect(new Set(heights).size).toBe(heights.length);
		expect(measures(1, "stacked")).toBe(measures(1));
	});

	// Kills a name or title that closes its attribute, opens a tag, or reads
	// back other than it was written.
	test("escapes every value it writes, quotes and angle brackets included", () => {
		const name = `Rösti "Fries" & 'Chips' <b>&quot;</b>`;
		const title = `"><script>alert('x')</script><i title="`;
		const html = embedCode({
			origin: "https://bencher.dev",
			slug: "rosti",
			query: QUERY,
			name,
			title,
		});
		expect(html.match(/</g)).toHaveLength(2);
		expect(html.match(/>/g)).toHaveLength(2);
		expect(html).not.toContain("'");
		const body = parse(html);
		expect(body.querySelectorAll("*")).toHaveLength(1);
		const frame = body.querySelector("iframe");
		expect(frame?.getAttribute("title")).toBe(`${title} | ${name}`);
		expect(
			decodeQuery(new URL(frame?.getAttribute("src") ?? "").search).sets,
		).toEqual(QUERY.sets);
	});
});
