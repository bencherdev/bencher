import { describe, expect, test } from "vitest";
import { ApiError } from "../api";
import { lastRefreshed, refusalOf } from "./apps";

const SESSION = "eyJhbGciOiJIUzI1NiJ9.e30.signature";
const KEY = `bencher_user_${"a".repeat(30)}`;

describe("refusalOf", () => {
	// Kills a server without OAuth shown as a failure to retry, a reader signed
	// in with a key told they lack access, and a failure worth retrying taken
	// for a refusal.
	test("names why the API will not list the apps", () => {
		const refused = (status: number, kind: ApiError["kind"]) =>
			new ApiError(status, kind, "{}");
		expect(refusalOf(refused(404, "not_found"), SESSION)).toBe(
			"Not available on this server",
		);
		expect(refusalOf(refused(403, "forbidden"), KEY)).toBe(
			"Not available with an API key",
		);
		expect(refusalOf(refused(403, "forbidden"), SESSION)).toBe("No access");
		expect(refusalOf(refused(500, "server"), SESSION)).toBeUndefined();
		expect(
			refusalOf(new ApiError(undefined, "network", "offline"), SESSION),
		).toBeUndefined();
		expect(refusalOf(new Error("boom"), SESSION)).toBeUndefined();
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
