// @vitest-environment happy-dom
// @vitest-environment-options {"url":"https://bencher.dev/perf/p?back=Ly4vL2V2aWwuY29t"}
import { expect, test } from "vitest";
import { decodePath } from "./url";

test("decodePath keeps a back path that normalizes to a leading double slash on this origin", () => {
	expect(decodePath("/fallback")).toBe("https://bencher.dev//evil.com");
});
