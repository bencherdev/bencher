import { expect, test } from "vitest";
import { NEXT_PROJECTS } from "../paths";
import { runPath } from "./path";

// Kills a run link outside the project's Runs.
test("runPath is the run under its project's Runs", () => {
	expect(runPath("hashbrown", "abc")).toBe(
		`${NEXT_PROJECTS}/hashbrown/runs/abc`,
	);
});
