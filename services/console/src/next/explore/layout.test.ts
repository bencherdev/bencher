import { describe, expect, test } from "vitest";
import { lineCapWarning, measuresLayout } from "./layout";

describe("measuresLayout", () => {
	// Kills offering the layout control for a single measure.
	test("offer no layout for one measure", () => {
		expect(measuresLayout(1, "stacked").control).toBe("none");
		expect(measuresLayout(0, "dual").control).toBe("none");
	});

	// Kills ignoring the reader's choice at two measures.
	test("follow the reader's choice at two measures", () => {
		expect(measuresLayout(2, "dual")).toEqual({
			layout: "dual",
			control: "choice",
		});
		expect(measuresLayout(2, "stacked")).toEqual({
			layout: "stacked",
			control: "choice",
		});
	});

	// Kills drawing three measures on two axes, or forcing the stack at two.
	test("force stacked at three or more measures", () => {
		for (const measures of [3, 8]) {
			expect(measuresLayout(measures, "dual")).toEqual({
				layout: "stacked",
				control: "forced",
			});
		}
	});
});

describe("lineCapWarning", () => {
	// Kills an off by one at either edge of the warning.
	test("warn near the cap, from 56 lines", () => {
		expect(lineCapWarning(55)).toBeUndefined();
		expect(lineCapWarning(56)).toBe("near");
		expect(lineCapWarning(64)).toBe("near");
	});

	// Kills calling a query that lost lines merely near the cap.
	test("say the query is over the cap past 64 lines", () => {
		expect(lineCapWarning(65)).toBe("over");
		expect(lineCapWarning(4_096)).toBe("over");
	});
});
