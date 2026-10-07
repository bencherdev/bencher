import { describe, expect, test } from "vitest";
import { decodeBase64 } from "../util/convert";
import { USER_KEY, isCurrent, readReader, signInHref } from "./reader";

const reader = {
	user: {
		uuid: "8c5b2b6e-4f3a-4c2d-9e1f-0a1b2c3d4e5f",
		name: "Muriel Bagge",
		slug: "muriel-bagge",
		email: "muriel.bagge@nowhere.com",
		admin: false,
		locked: false,
	},
	token: "header.payload.signature",
	creation: "2026-09-01T00:00:00Z",
	expiration: "2026-10-01T00:00:00Z",
};

const storage = (value: string | null) => ({
	getItem: (key: string) => (key === USER_KEY ? value : null),
});

describe("readReader", () => {
	// Kills a console that treats a missing, broken, or tokenless user as signed in.
	test("reads only a stored user with a token", () => {
		expect(readReader(storage(JSON.stringify(reader)))).toEqual(reader);
		expect(readReader(storage(null))).toBeUndefined();
		expect(readReader(storage("{"))).toBeUndefined();
		expect(
			readReader(storage(JSON.stringify({ ...reader, token: "" }))),
		).toBeUndefined();
		expect(
			readReader(storage(JSON.stringify({ ...reader, user: {} }))),
		).toBeUndefined();
	});
});

describe("isCurrent", () => {
	// Kills an expired session treated as signed in.
	test("a session ends at its expiration", () => {
		expect(isCurrent(reader, Date.parse("2026-09-30T23:59:59Z"))).toBe(true);
		expect(isCurrent(reader, Date.parse("2026-10-01T00:00:00Z"))).toBe(false);
		expect(isCurrent({ ...reader, expiration: "never" }, 0)).toBe(false);
	});
});

describe("signInHref", () => {
	// Kills a way back that the classic sign in cannot read.
	test("carries the way back the classic console reads", () => {
		const path = "/next/console/projects/hashbrown/reports?page=2#top";
		const href = new URL(signInHref(path), "https://bencher.dev");
		expect(href.pathname).toBe("/auth/login");
		expect(decodeBase64(href.searchParams.get("back"))).toBe(path);
	});
});
