import { describe, expect, test } from "vitest";
import { onlyIteration, rangeBars, withThrough } from "./iterations";
import { DAY, RUNS, START } from "./testing";

describe("withThrough", () => {
	// Kills a line through the first iteration, or past the one that alerted, and a column made for data run once.
	test("gives each line its column and leaves one run once as it came", () => {
		const { iteration: _iteration, ...once } = RUNS;
		expect(withThrough(once)).toBe(once);
		expect(withThrough(RUNS).lines[0]?.through).toEqual([
			10,
			null,
			null,
			12,
			null,
			13,
			null,
		]);
	});
});

describe("rangeBars", () => {
	// Kills a bar for a report run once and a bar that misses an iteration.
	test("spans each report's iterations from the lowest to the highest", () => {
		const [line] = RUNS.lines;
		expect(rangeBars(line?.y ?? [], RUNS.report)).toEqual([
			{ index: 1, low: 11, high: 14 },
			{ index: 4, low: 9, high: 13 },
		]);
		expect(rangeBars([1, 2], [0, 1])).toEqual([]);
		expect(rangeBars([1, null, 2], [0, 0, 1])).toEqual([]);
	});
});

describe("onlyIteration", () => {
	// Kills counting iterations from 0, points of other iterations kept, and alerts left at their old index.
	test("keeps only the points of the chosen iteration, counting from 1", () => {
		const second = onlyIteration(RUNS, 2);
		expect(second.x).toEqual([START + DAY, START + 2 * DAY]);
		expect(second.report).toEqual([1, 2]);
		expect(second.iteration).toEqual([1, 1]);
		expect(second.lines[0]?.y).toEqual([14, 13]);
		expect(second.lines[0]?.upper).toEqual([15, 12]);
		expect(second.lines[0]?.alerts).toEqual([1]);
		expect(second.lines[0]?.raised).toEqual([
			{ index: 1, uuid: "alert", status: "dismissed" },
		]);
		expect(onlyIteration(RUNS, 1).lines[0]?.y).toEqual([10, 11, 10]);
		expect(onlyIteration(RUNS, 1).lines[0]?.alerts).toEqual([]);
	});
});
