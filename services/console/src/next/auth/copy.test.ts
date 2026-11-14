import { describe, expect, test } from "vitest";
import {
	VERIFICATION_FAILED,
	capped,
	doorFailure,
	duration,
	inboxFailure,
	providerFailure,
} from "./copy";
import type { Refused, RefusalCode } from "./request";

const refused = (
	code: RefusalCode | undefined,
	retryAfter?: number,
): Refused => ({ status: 403, code, retryAfter, triesLeft: undefined });

describe("the door's failures", () => {
	// Kills a refusal the reader cannot tell apart from any other, a Try again
	// offered where trying again cannot help, and a wait that is not the API's.
	test("each refusal says what went wrong", () => {
		expect(doorFailure(refused("turnstile_failed"))).toBe(VERIFICATION_FAILED);
		expect(doorFailure(refused("email_failed"))).toEqual({
			text: "The email did not send",
			retry: true,
		});
		expect(doorFailure(refused("too_many_attempts", 90))).toEqual({
			text: "Too many attempts. Try again in 2 minutes.",
			retry: false,
		});
		expect(doorFailure(refused("invalid_invite"))).toEqual({
			text: "This invite is not valid",
			retry: false,
		});
		expect(doorFailure(refused("invite_for_another_email"))).toEqual({
			text: "This invite is for another email",
			retry: false,
		});
		expect(doorFailure(refused("unknown_organization"))).toEqual({
			text: "This organization no longer exists",
			retry: false,
		});
	});

	// A 500 carries an issue's ID where a refusal's code goes. Kills a page
	// that reads a code it does not know, or none, as anything but a failure.
	test("any other refusal is the generic failure", () => {
		for (const code of [undefined, "cross_origin", "wrong_code"] as const) {
			expect(doorFailure(refused(code))).toEqual({
				text: "Sign-in failed",
				retry: true,
			});
		}
	});
});

describe("Check your inbox's failures", () => {
	// Kills a locked account, a refused check, or a wait the reader cannot see.
	test("each refusal says what went wrong", () => {
		expect(inboxFailure(refused("turnstile_failed"))).toBe(VERIFICATION_FAILED);
		expect(inboxFailure(refused("locked")).text).toBe("This account is locked");
		expect(inboxFailure(refused("too_many_attempts", 30)).text).toBe(
			"Too many attempts. Try again in 1 minute.",
		);
		expect(inboxFailure(refused("too_many_attempts")).text).toBe(
			"Too many attempts. Try again later.",
		);
		expect(inboxFailure(refused(undefined))).toEqual({
			text: "Sign-in failed",
			retry: true,
		});
	});
});

describe("a provider callback's failures", () => {
	// Kills a refusal by the API that reads as the provider's own, a locked
	// account or a wait the reader cannot see, and a code this page does not
	// know read as anything but the generic failure.
	test("each refusal says what went wrong, and any other is the generic failure", () => {
		expect(providerFailure(refused("locked"))).toBe("This account is locked");
		expect(providerFailure(refused("too_many_attempts", 90))).toBe(
			"Too many attempts. Try again in 2 minutes.",
		);
		for (const code of [
			"provider_refused",
			undefined,
			"cross_origin",
			"wrong_code",
		] as const) {
			expect(providerFailure(refused(code))).toBe("Sign-in failed");
		}
	});
});

describe("duration", () => {
	// Kills a wait rounded down, which tells the reader to try too soon, and
	// one in units the reader must convert.
	test("rounds up to whole minutes, then whole hours", () => {
		expect(
			[1, 60, 61, 59 * 60 + 1, 60 * 60, 60 * 60 + 1, 23 * 60 * 60 + 1].map(
				duration,
			),
		).toEqual([
			"1 minute",
			"1 minute",
			"2 minutes",
			"1 hour",
			"1 hour",
			"2 hours",
			"24 hours",
		]);
		expect(capped(40 * 60)).toBe(
			"Too many emails to this address. Try again in 40 minutes.",
		);
	});
});
