import { describe, expect, test } from "vitest";
import { landing } from "./landing";
import { projectAt } from "./home";
import { projectPath } from "./paths";

const READER = JSON.stringify({
	uuid: "u",
	slug: "muriel-bagge",
	name: "Muriel Bagge",
});
const versions = (entries: Record<string, number>) => JSON.stringify(entries);
const HASHBROWN = versions({ hashbrown: 1 });

describe("landing", () => {
	// Kills a landing that sends a reader who is not signed in to a console
	// that cannot load.
	test("a reader who is not signed in stays", () => {
		expect(landing(null, HASHBROWN, "", projectAt)).toBeUndefined();
		expect(landing("{}", HASHBROWN, "", projectAt)).toBeUndefined();
		expect(landing("{", HASHBROWN, "", projectAt)).toBeUndefined();
	});

	// Kills a landing that drops an invite, a claim, or a plan before the
	// page can act on it.
	test("a query string stays", () => {
		expect(
			landing(READER, HASHBROWN, "?invite=abc", projectAt),
		).toBeUndefined();
	});

	// Kills a landing that sends a reader with no main project anywhere.
	test("no memory stays", () => {
		expect(landing(READER, null, "", projectAt)).toBeUndefined();
		expect(landing(READER, "{}", "", projectAt)).toBeUndefined();
		expect(landing(READER, "{", "", projectAt)).toBeUndefined();
	});

	// Kills a version 1 project sent anywhere but its root in the new console.
	test("a version 1 project lands on its root", () => {
		expect(landing(READER, HASHBROWN, "", projectAt)).toBe(
			projectPath("hashbrown"),
		);
	});

	// Kills a version 0 project sent anywhere but its root in the console.
	test("a version 0 project lands on its root", () => {
		expect(landing(READER, versions({ hashbrown: 0 }), "", projectAt)).toBe(
			projectPath("hashbrown"),
		);
	});

	// Kills a landing on the first project remembered rather than the last
	// one visited, and one that stops at an entry that names no version.
	test("the most recent project wins", () => {
		expect(
			landing(READER, versions({ hashbrown: 0, crinkle: 1 }), "", projectAt),
		).toBe(projectPath("crinkle"));
		expect(
			landing(READER, versions({ hashbrown: 1, crinkle: 2 }), "", projectAt),
		).toBe(projectPath("hashbrown"));
	});
});
