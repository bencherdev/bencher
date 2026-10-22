import { expect, test } from "vitest";
import { sectionOf } from "./section";

// Kills a path drawn as the wrong section, Keys or Connections left unserved
// at its path, Keys still served at its old one, and an unknown path drawn as
// General.
test("each path under Settings belongs to one section", () => {
	expect(sectionOf(["settings"])).toBe("general");
	expect(sectionOf(["keys"])).toBe("keys");
	expect(sectionOf(["connections"])).toBe("connections");
	expect(sectionOf(["settings", "keys"])).toBeUndefined();
	expect(sectionOf(["settings", "nope"])).toBeUndefined();
	expect(sectionOf(["keys", "abc"])).toBeUndefined();
	expect(sectionOf(["connections", "github"])).toBeUndefined();
});
