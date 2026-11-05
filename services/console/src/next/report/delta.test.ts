import { describe, expect, test } from "vitest";
import { guardOf } from "./delta";

describe("guardOf", () => {
	// Kills reading one boundary for the other, or dropping the two-sided case.
	test.each([
		[{ upper_boundary: 0.99 }, "upper"],
		[{ lower_boundary: 0.99 }, "lower"],
		[{ lower_boundary: 0.95, upper_boundary: 0.99 }, "both"],
	] as const)("a model with %o guards %s", (model, guard) => {
		expect(guardOf(model)).toBe(guard);
	});

	// Kills treating a line no threshold checked as guarded.
	test("a line no threshold checked has no guarded side", () => {
		expect(guardOf(undefined)).toBeUndefined();
	});
});
