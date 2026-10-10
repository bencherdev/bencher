import { describe, expect, test } from "vitest";
import { batchQuery, countQuery, isReportId } from "./query";
import { fakeApi } from "../reports/testing";
import { reportFixture } from "./testing";
import type { ReportView } from "./view";

const VIEW: ReportView = {
	group: "benchmark",
	sort: "name",
	window: "4w",
	search: "",
	expanded: [],
};

/** An API that answers every read with one batch and records what it was asked. */
const recorder = () => {
	const { api, requests } = fakeApi(() => ({ data: reportFixture() }));
	return { api, paths: requests };
};

describe("batchQuery", () => {
	// Kills a window sent as its preset name, a page the API reads as the first, or a lost search.
	test("asks for one batch of the view's lines", async () => {
		const { api, paths } = recorder();
		const query = batchQuery(
			api,
			"hash brown",
			"r1",
			{
				...VIEW,
				group: "measure",
				sort: "delta",
				window: "3m",
				search: "avx2",
			},
			{ page: 3, perPage: 24 },
		);
		await query.queryFn({ signal: new AbortController().signal });
		const [url] = paths;
		expect(url?.pathname).toBe("/v0/projects/hash%20brown/console/reports/r1");
		expect(Object.fromEntries(url?.searchParams ?? [])).toEqual({
			window: "92",
			group: "measure",
			sort: "delta",
			search: "avx2",
			page: "3",
			per_page: "24",
		});
	});

	// Kills the API's defaults written into every request, or a custom window lost.
	test("leaves the default grouping and sort to the API", async () => {
		const { api, paths } = recorder();
		await batchQuery(
			api,
			"hashbrown",
			"r1",
			{ ...VIEW, window: 10 },
			{ page: 1, perPage: 24 },
		).queryFn({ signal: new AbortController().signal });
		expect(paths[0]?.search).toBe("?window=10&page=1&per_page=24");
	});

	// Kills a cache entry shared by two views, or one split by which rows are open.
	test("keys a batch by the view's lines, not its open rows", () => {
		const key = (view: ReportView) =>
			JSON.stringify(
				batchQuery({} as never, "p", "r", view, { page: 1, perPage: 9 })
					.queryKey,
			);
		expect(key({ ...VIEW, expanded: ["a"] })).toBe(key(VIEW));
		expect(key({ ...VIEW, sort: "delta" })).not.toBe(key(VIEW));
		expect(key({ ...VIEW, search: "x" })).not.toBe(key(VIEW));
	});

	// Kills another report's lines held on screen, or a later batch held over the new first one.
	test("holds the rows on screen through a new view of the same report only", () => {
		const previous = { report: reportFixture(), group: "benchmark" as const };
		const placeholder = (report: string, page: number) =>
			batchQuery({} as never, "p", report, VIEW, {
				page,
				perPage: 9,
			}).placeholderData(previous, {
				queryKey: ["console", "report", "p", "r", "", 9, 1],
			});
		expect(placeholder("r", 1)).toBe(previous);
		expect(placeholder("other", 1)).toBeUndefined();
		expect(placeholder("r", 2)).toBeUndefined();
	});
});

describe("countQuery", () => {
	// Kills a count that reads lines, or one that drops the measure, metric, or parameters.
	test("counts the lines a threshold would check without reading any", async () => {
		const { api, paths } = recorder();
		await countQuery(api, "hashbrown", "r1", {
			measure: "latency-uuid",
			metric: "p 99",
			parameters: { threads: 1, simd: "avx2" },
		}).queryFn({ signal: new AbortController().signal });
		expect(Object.fromEntries(paths[0]?.searchParams ?? [])).toEqual({
			per_page: "0",
			measure: "latency-uuid",
			metric: "p 99",
			parameters: '{"simd":"avx2","threads":1}',
		});
	});
});

describe("isReportId", () => {
	// Kills a check that lets a truncated hash or a stray character reach the API, or turns away a UUID it reads.
	test("names a UUID, hyphenated or simple, in either case, and nothing else", () => {
		for (const id of [
			"9c1f2e40-5a7b-4c3d-8e9f-0a1b2c3d4e5f",
			"9C1F2E40-5A7B-4C3D-8E9F-0A1B2C3D4E5F",
			"9c1f2e405a7b4c3d8e9f0a1b2c3d4e5f",
		]) {
			expect(isReportId(id)).toBe(true);
		}
		for (const id of [
			"9c1f2e4",
			"9c1f2e40-5a7b-4c3d-8e9f-0a1b2c3d4e5",
			"9c1f2e40-5a7b4c3d-8e9f-0a1b2c3d4e5f",
			"9c1f2e40-5a7b-4c3d-8e9f-0a1b2c3d4e5g",
			" 9c1f2e40-5a7b-4c3d-8e9f-0a1b2c3d4e5f",
		]) {
			expect(isReportId(id)).toBe(false);
		}
	});
});
