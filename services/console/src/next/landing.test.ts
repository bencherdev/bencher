import { describe, expect, test } from "vitest";
import { landing } from "./landing";
import { projectPath } from "./paths";

const NOW = Date.parse("2026-09-14T00:00:00Z");
const READER = JSON.stringify({
	token: "tok",
	user: { uuid: "u" },
	expiration: "2026-09-15T00:00:00Z",
});
const versions = (entries: Record<string, number>) => JSON.stringify(entries);
const HASHBROWN = versions({ hashbrown: 1 });

describe("landing", () => {
	// Kills a landing that sends a reader who is not signed in to a console
	// that cannot load.
	test("a reader who is not signed in stays", () => {
		const noToken = JSON.stringify({
			token: "",
			user: { uuid: "u" },
			expiration: "2026-09-15T00:00:00Z",
		});
		expect(landing(null, HASHBROWN, "", NOW)).toBeUndefined();
		expect(landing(noToken, HASHBROWN, "", NOW)).toBeUndefined();
		expect(landing("{", HASHBROWN, "", NOW)).toBeUndefined();
	});

	// Kills a landing that trusts a token past its expiration.
	test("a reader whose token has expired stays", () => {
		const expired = JSON.stringify({
			token: "tok",
			user: { uuid: "u" },
			expiration: "2026-09-13T23:59:59Z",
		});
		expect(landing(expired, HASHBROWN, "", NOW)).toBeUndefined();
	});

	// Kills an expiration check that lets a token through at its expiration.
	test("a reader whose token expires now stays", () => {
		const expiring = JSON.stringify({
			token: "tok",
			user: { uuid: "u" },
			expiration: new Date(NOW).toISOString(),
		});
		expect(landing(expiring, HASHBROWN, "", NOW)).toBeUndefined();
	});

	// Kills a token check that takes any truthy token.
	test("a reader whose token is not a string stays", () => {
		const numeric = JSON.stringify({
			token: 1,
			user: { uuid: "u" },
			expiration: "2026-09-15T00:00:00Z",
		});
		expect(landing(numeric, HASHBROWN, "", NOW)).toBeUndefined();
	});

	// Kills a landing that drops an invite, a claim, or a plan before the
	// page can act on it.
	test("a query string stays", () => {
		expect(landing(READER, HASHBROWN, "?invite=abc", NOW)).toBeUndefined();
	});

	// Kills a landing that sends a reader with no main project anywhere.
	test("no memory stays", () => {
		expect(landing(READER, null, "", NOW)).toBeUndefined();
		expect(landing(READER, "{}", "", NOW)).toBeUndefined();
		expect(landing(READER, "{", "", NOW)).toBeUndefined();
	});

	// Kills a version 1 project sent anywhere but its root in the new console.
	test("a version 1 project lands on its root", () => {
		expect(landing(READER, HASHBROWN, "", NOW)).toBe(projectPath("hashbrown"));
	});

	// Kills a version 0 project sent to the new console, which would send it
	// back.
	test("a version 0 project lands on its classic page", () => {
		expect(landing(READER, versions({ hashbrown: 0 }), "", NOW)).toBe(
			"/console/projects/hashbrown/plots",
		);
	});

	// Kills a landing on the first project remembered rather than the last
	// one visited, and one that stops at an entry that names no version.
	test("the most recent project wins", () => {
		expect(
			landing(READER, versions({ hashbrown: 0, crinkle: 1 }), "", NOW),
		).toBe(projectPath("crinkle"));
		expect(
			landing(READER, versions({ hashbrown: 1, crinkle: 2 }), "", NOW),
		).toBe(projectPath("hashbrown"));
	});
});
