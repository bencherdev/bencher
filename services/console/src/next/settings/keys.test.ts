import { describe, expect, test } from "vitest";
import type {
	JsonProjectKey,
	JsonProjectKeyCreated,
} from "../../types/bencher";
import { EXPIRIES, expiresAt, formatDay, keyEnd, listed } from "./keys";

const SEP_13 = Date.parse("2026-09-13T15:00:00Z");
const NEVER_SECONDS = 2 ** 32 - 1;

const key = (
	name: string,
	creation: string,
	expiration: string,
	revoked?: string,
): JsonProjectKey => ({
	uuid: `uuid-${name}`,
	project: "p",
	name,
	creation,
	expiration,
	...(revoked ? { revoked } : {}),
});

describe("the expiry choices", () => {
	// Kills a choice whose day does not match its label, and a Never that ends.
	test("each choice names the day it ends on", () => {
		expect(
			EXPIRIES.map((expiry) => {
				const at = expiresAt(SEP_13, expiry);
				return [expiry.label, at === undefined ? "" : formatDay(at, "UTC")];
			}),
		).toEqual([
			["30 days", "Oct 13, 2026"],
			["90 days", "Dec 12, 2026"],
			["1 year", "Sep 13, 2027"],
			["Never", ""],
		]);
	});
});

describe("keyEnd", () => {
	const now = Date.parse("2026-09-13T00:00:00Z");

	// Kills a key with no time to live shown with a date 136 years out, and a
	// long one taken for Never.
	test("a key made with no time to live never expires", () => {
		const creation = "2026-09-02T10:00:00Z";
		const never = new Date(
			Date.parse(creation) + NEVER_SECONDS * 1000,
		).toISOString();
		const almost = new Date(
			Date.parse(creation) + (NEVER_SECONDS - 1) * 1000,
		).toISOString();
		expect(keyEnd(key("a", creation, never), now, "UTC")).toEqual({
			text: "Never",
			expired: false,
		});
		expect(keyEnd(key("b", creation, almost), now, "UTC").text).not.toBe(
			"Never",
		);
	});

	// Kills an expired key shown as if it still worked.
	test("names the day a key ends, and says when it already has", () => {
		expect(
			keyEnd(
				key("a", "2026-09-02T10:00:00Z", "2026-10-02T10:00:00Z"),
				now,
				"UTC",
			),
		).toEqual({ text: "Oct 2, 2026", expired: false });
		expect(
			keyEnd(
				key("b", "2026-07-01T10:00:00Z", "2026-09-01T10:00:00Z"),
				now,
				"UTC",
			),
		).toEqual({ text: "Expired Sep 1, 2026", expired: true });
	});
});

describe("listed", () => {
	// Kills the shown-once secret written into the list, which the cache keeps.
	test("a new key joins the list without its secret", () => {
		const created: JsonProjectKeyCreated = {
			uuid: "u",
			project: "p",
			name: "release-benchmarks",
			key: "bencher_run_7fK2xQ9mVb4LtR8nWc3Hy6PdJs1Ga5ZeUo0iXq",
			creation: "2026-09-13T00:00:00Z",
			expiration: "2026-12-12T00:00:00Z",
		};
		const row = listed(created);
		expect(JSON.stringify(row)).not.toContain(created.key);
		expect(row).toEqual({
			uuid: "u",
			name: "release-benchmarks",
			creation: "2026-09-13T00:00:00Z",
			expiration: "2026-12-12T00:00:00Z",
		});
	});
});
