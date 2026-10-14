import { describe, expect, test } from "vitest";
import { AppsRefusal, appsRefusal, lastRefreshed } from "./apps";

const SESSION = "eyJhbGciOiJIUzI1NiJ9.e30.signature";
const KEY = `bencher_user_${"a".repeat(30)}`;

describe("appsRefusal", () => {
	// Kills a page that tells a user signed in with a key they lack access,
	// or a server without OAuth that it failed.
	test("names why the apps could not be listed", () => {
		expect(appsRefusal(404, SESSION)).toBe(AppsRefusal.Unavailable);
		expect(appsRefusal(403, KEY)).toBe(AppsRefusal.KeyRefused);
		expect(appsRefusal(403, SESSION)).toBe(AppsRefusal.Forbidden);
		expect(appsRefusal(400, SESSION)).toBe(AppsRefusal.SessionEnded);
		expect(appsRefusal(401, SESSION)).toBe(AppsRefusal.SessionEnded);
		expect(appsRefusal(429, SESSION)).toBe(AppsRefusal.Failed);
		expect(appsRefusal(undefined, SESSION)).toBe(AppsRefusal.Failed);
	});
});

describe("lastRefreshed", () => {
	const created = "2026-10-01T12:00:00Z";

	// Kills a page that shows when the app connected as when it last refreshed.
	test("only after the app renewed its access", () => {
		expect(lastRefreshed({ created, modified: created })).toBeUndefined();
		expect(lastRefreshed({ created, modified: "2026-10-02T12:00:00Z" })).toBe(
			"2026-10-02T12:00:00Z",
		);
	});
});
