import { describe, expect, test } from "vitest";
import { checkoutReturn } from "./checkout";

const SELF_HOSTED = "00000000-0000-4000-8000-000000000001";

describe("checkoutReturn", () => {
	// Kills a return whose session, level, entitlements, or self-hosted
	// organization does not reach the plan the API records.
	test("reads the plan Stripe's return names", () => {
		expect(checkoutReturn("?checkout=cs_test_1&level=pro")).toEqual({
			checkout: "cs_test_1",
			level: "pro",
		});
		expect(
			checkoutReturn(
				`?checkout=cs_test_2&level=enterprise&entitlements=4294967295&self_hosted=${SELF_HOSTED}`,
			),
		).toEqual({
			checkout: "cs_test_2",
			level: "enterprise",
			entitlements: 4_294_967_295,
			self_hosted: SELF_HOSTED,
		});
	});

	// Kills Billing recording a plan from the pricing page's link, which only
	// starts a checkout.
	test("reads nothing from the pricing page's link", () => {
		expect(checkoutReturn("?plan=pro")).toBeUndefined();
	});
});
