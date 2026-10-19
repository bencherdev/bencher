import { describe, expect, test } from "vitest";
import { rememberedHome } from "./home";
import { VERSION_KEY } from "./memory";
import { projectPath } from "./paths";

const versions = (entries: Record<string, number>) => ({
	getItem: (key: string) =>
		key === VERSION_KEY ? JSON.stringify(entries) : null,
});

describe("rememberedHome", () => {
	// Kills a home that is not the most recently loaded project, one opened
	// anywhere but where its version opens, and a guess when nothing is known.
	test("is the newest remembered project where its version opens", () => {
		expect(rememberedHome(versions({ older: 0, newer: 1 }))).toBe(
			projectPath("newer"),
		);
		expect(rememberedHome(versions({ older: 1, newer: 0 }))).toBe(
			"/console/projects/newer/plots",
		);
		expect(rememberedHome({ getItem: () => null })).toBeUndefined();
	});
});
