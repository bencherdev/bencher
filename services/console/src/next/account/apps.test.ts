import { describe, expect, test } from "vitest";
import { ApiError } from "../api";
import { lastRefreshed, refusalOf } from "./apps";

describe("refusalOf", () => {
	// Kills a server without OAuth shown as a failure to retry, and a failure
	// worth retrying taken for a refusal.
	test("names why the API will not list the apps", () => {
		const refused = (status: number, kind: ApiError["kind"]) =>
			new ApiError(status, kind, "{}");
		expect(refusalOf(refused(404, "not_found"))).toBe(
			"Not available on this server",
		);
		expect(refusalOf(refused(403, "forbidden"))).toBe("No access");
		expect(refusalOf(refused(500, "server"))).toBeUndefined();
		expect(
			refusalOf(new ApiError(undefined, "network", "offline")),
		).toBeUndefined();
		expect(refusalOf(new Error("boom"))).toBeUndefined();
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
