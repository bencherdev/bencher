import { expect, test } from "vitest";
import { placeOf } from "./dimension";

// Kills a list drawn for a page's path, a page for a deeper path, and a path
// outside the four dimensions taken for one.
test("a path names a dimension's list, or one dimension's page", () => {
	expect(placeOf(["branches"])).toEqual({ dimension: "branches" });
	expect(placeOf(["measures", "latency"])).toEqual({
		dimension: "measures",
		entry: "latency",
	});
	expect(placeOf(["testbeds", "linux", "edit"])).toBeUndefined();
	expect(placeOf(["settings", "dimensions"])).toBeUndefined();
	expect(placeOf([])).toBeUndefined();
});
