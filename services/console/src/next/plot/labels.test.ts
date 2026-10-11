import { expect, test } from "vitest";
import { axisFont, axisWidth } from "./labels";

// Node resolves Solid's server build, where there is no document to measure with.
// Kills measuring on the server, which would throw before a public plot could render there.
test("takes the narrowest axis on the server", () => {
	expect(axisWidth(["1,000,000.25"], axisFont("monospace"))).toBe(30);
});
