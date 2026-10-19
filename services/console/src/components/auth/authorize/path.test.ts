import { describe, expect, test } from "vitest";
import { consentPath, signInPath, switchPath } from "./path";

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

describe("switchPath", () => {
	// Kills a switch between log in and sign up that drops the way back, or the
	// request beside it, or sends a way back that is not one.
	test("carries the request and the way back", () => {
		expect(switchPath("/auth/signup", UUID, "L2NvbnNvbGU+Lw==")).toBe(
			`/auth/signup?authorization=${UUID}&back=L2NvbnNvbGU%2BLw%3D%3D`,
		);
		expect(switchPath("/auth/login", null, "L2NvbnNvbGU=")).toBe(
			"/auth/login?back=L2NvbnNvbGU%3D",
		);
		for (const back of [null, ""]) {
			expect(switchPath("/auth/login", UUID, back)).toBe(
				`/auth/login?authorization=${UUID}`,
			);
		}
	});
});
