import { describe, expect, test } from "vitest";
import { checkoutReturn } from "./checkout";

describe("checkoutReturn", () => {
	// Kills a return whose session does not reach the plan the API records, or one
	// that still sends the level, entitlements, or self-hosted organization an
	// earlier return named.
	test("reads the session Stripe's return names", () => {
		expect(checkoutReturn("?checkout=cs_test_1")).toEqual({
			checkout: "cs_test_1",
		});
		expect(
			checkoutReturn(
				"?checkout=cs_test_2&level=enterprise&entitlements=4294967295&self_hosted=00000000-0000-4000-8000-000000000001",
			),
		).toEqual({ checkout: "cs_test_2" });
	});

	// Kills Billing recording a plan from the pricing page's link, which only
	// starts a checkout, or from a link with no session.
	test("reads nothing from a link without a session", () => {
		expect(checkoutReturn("?plan=pro")).toBeUndefined();
		expect(checkoutReturn("?level=pro")).toBeUndefined();
		expect(checkoutReturn("?checkout=")).toBeUndefined();
	});
});
