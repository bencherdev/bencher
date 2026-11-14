import { describe, expect, test } from "vitest";
import { carryOf } from "./params";

describe("carryOf", () => {
	// Kills a door that sends the API a value of the wrong shape, which it would
	// refuse with no way for the reader to fix it, drops one of the right shape,
	// or reads an invitation from the query, where a server's log would keep it.
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
