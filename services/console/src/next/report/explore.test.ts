import { describe, expect, test } from "vitest";
import { decodeQuery } from "../query/query";
import { exploreSearchOf, selectedLine } from "./explore";
import { linesOf } from "./lines";
import { REPORT, lineFixture, reportFixture } from "./testing";

describe("selectedLine", () => {
	const batch = reportFixture([
		lineFixture({
			alert: { uuid: "a", limit: "upper", status: "active" } as never,
		}),
		lineFixture({ variant: 1, measure: 1, model: undefined }),
	]);
	const lines = linesOf(batch);

	// Kills a selection drawn from the branch's current head, or that loses its alert.
	test("opens a line from the report's head", () => {
		expect(selectedLine(lines[0] as never)).toEqual({
			branch: { uuid: "branch", head: "head" },
			testbed: { uuid: "testbed" },
			benchmark: "blake3-uuid",
			parameters: { input_bytes: 65536, simd: "avx2", threads: 1 },
			measure: "latency-uuid",
			metric: "value",
			alerting: true,
		});
	});

	// Kills a selection whose lines Explore would key differently from the rows,
	// and one that loses the window ending at the report or the report itself.
	test("opens exactly the selected lines in Explore, over the report's window", () => {
		const end = Date.parse("2026-09-13T21:16:00Z");
		const query = decodeQuery(
			exploreSearchOf(lines, { uuid: REPORT, end }, 92),
		);
		expect(query.only).toEqual(lines.map(({ key }) => key));
		expect(query.window).toEqual({ seconds: 92 * 86_400, end });
		expect(query.report).toBe(REPORT);
	});
});
