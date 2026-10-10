import { expect, test } from "vitest";
import { sectionOf } from "./section";

// Kills a path drawn as the wrong section, the classic keys path left
// unserved, and an unknown path drawn as General.
test("each path under Settings belongs to one section", () => {
	expect(sectionOf(["settings"])).toBe("general");
	expect(sectionOf(["settings", "keys"])).toBe("keys");
	expect(sectionOf(["keys"])).toBe("keys");
	expect(sectionOf(["settings", "dimensions"])).toBe("dimensions");
	expect(sectionOf(["branches", "main"])).toBe("dimensions");
	expect(sectionOf(["measures"])).toBe("dimensions");
	expect(sectionOf(["settings", "nope"])).toBeUndefined();
	expect(sectionOf(["keys", "abc"])).toBeUndefined();
});
