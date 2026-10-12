import { describe, expect, test } from "vitest";
import { consentPath, signInPath } from "./path";

const UUID = "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f";

describe("consentPath", () => {
	// Kills a sign in that returns anywhere but the consent page of one request.
	test("is the consent page of a valid request only", () => {
		expect(consentPath(UUID)).toBe(`/auth/authorize?request=${UUID}`);
		for (const value of [
			undefined,
			"",
			"/console",
			"https://evil.example",
			`${UUID}&request=${UUID}`,
		]) {
			expect(consentPath(value)).toBeUndefined();
		}
	});
});

describe("signInPath", () => {
	// Kills a sign in page that drops the request, or carries anything but one.
	test("carries a valid request only", () => {
		expect(signInPath("/auth/signup", UUID)).toBe(
			`/auth/signup?authorization=${UUID}`,
		);
		for (const value of [null, "", "not-a-uuid", `${UUID}&plan=team`]) {
			expect(signInPath("/auth/signup", value)).toBe("/auth/signup");
		}
	});
});
