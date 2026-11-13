import { describe, expect, test } from "vitest";
import { tokenEmail } from "./oldLink";

const base64url = (text: string) =>
	btoa(String.fromCodePoint(...new TextEncoder().encode(text)))
		.replace(/\+/g, "-")
		.replace(/\//g, "_")
		.replace(/=+$/, "");
const token = (claims: unknown) =>
	`${base64url('{"alg":"HS256"}')}.${base64url(JSON.stringify(claims))}.c2ln`;

describe("tokenEmail", () => {
	// Kills an old link's address not shown, one read from the wrong claim, and
	// a broken token that breaks the page.
	test("reads the address a token names", () => {
		expect(tokenEmail(token({ sub: "grace@example.com", aud: "auth" }))).toBe(
			"grace@example.com",
		);
		expect(tokenEmail(token({ sub: "名前@example.com" }))).toBe(
			"名前@example.com",
		);
		for (const broken of [
			undefined,
			"",
			"a.b.c",
			token({ email: "grace@example.com" }),
			token({ sub: 7 }),
		]) {
			expect(tokenEmail(broken), String(broken)).toBeUndefined();
		}
	});
});
