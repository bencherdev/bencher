import { describe, expect, test } from "vitest";
import { carryOf, tokenEmail } from "./params";

const base64url = (text: string) =>
	btoa(String.fromCodePoint(...new TextEncoder().encode(text)))
		.replace(/\+/g, "-")
		.replace(/\//g, "_")
		.replace(/=+$/, "");
const invite = (claims: unknown) =>
	`${base64url('{"alg":"HS256"}')}.${base64url(JSON.stringify(claims))}.c2ln`;

describe("carryOf", () => {
	// Kills a door that sends the API a value of the wrong shape, which it would
	// refuse with no way for the reader to fix it, or drops one of the right shape.
	test("carries each part only in its own shape", () => {
		const request = "0B5C4F6E-2A1D-4C3B-9E8F-7A6B5C4D3E2F";
		const claim = "C1A1D000-0000-4000-8000-000000000000";
		expect(
			carryOf(
				new URLSearchParams({
					invite: "a.b.c",
					claim,
					plan: "pro",
					authorization: request,
				}),
			),
		).toEqual({
			invite: "a.b.c",
			claim: claim.toLowerCase(),
			plan: "pro",
			authorization: request.toLowerCase(),
		});
		expect(
			carryOf(
				new URLSearchParams({
					invite: "not a token",
					claim: "nope",
					plan: "platinum",
					authorization: "nope",
				}),
			),
		).toEqual({});
	});
});

describe("tokenEmail", () => {
	// Kills an invite's address not filled in, or an old link's not shown, one
	// read from the wrong claim, and a broken token that breaks the page.
	test("reads the address a token names", () => {
		expect(
			tokenEmail(invite({ sub: "grace@example.com", aud: "invite" })),
		).toBe("grace@example.com");
		expect(tokenEmail(invite({ sub: "名前@example.com" }))).toBe(
			"名前@example.com",
		);
		for (const broken of [
			undefined,
			"",
			"a.b.c",
			invite({ email: "grace@example.com" }),
			invite({ sub: 7 }),
		]) {
			expect(tokenEmail(broken), String(broken)).toBeUndefined();
		}
	});
});
