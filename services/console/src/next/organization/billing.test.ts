import { describe, expect, test } from "vitest";
import type { JsonPriceTier } from "../../types/bencher";
import type { Api } from "../api";
import { bandIndex, bands, estimate, period, usageQuery, usd } from "./billing";

const UTC = "UTC";

// As the API sends them: an absent amount or bound is null, and the open band
// carries a placeholder price Stripe requires.
const PRO = JSON.parse(`[
	{ "up_to": 250, "unit_amount": null, "flat_amount": 10000 },
	{ "up_to": 375, "unit_amount": null, "flat_amount": 15000 },
	{ "up_to": 500, "unit_amount": null, "flat_amount": 20000 },
	{ "up_to": null, "unit_amount": null, "flat_amount": 50000 }
]`) as JsonPriceTier[];

describe("period", () => {
	// Kills a period that repeats its year, drops the end's, or joins with a dash.
	test("a period within one year names the year once, at its end", () => {
		expect(period("2026-10-01T00:00:00Z", "2026-10-31T00:00:00Z", UTC)).toBe(
			"Oct 1 to Oct 31, 2026",
		);
	});

	// Kills a period across a new year that names only the end's year.
	test("a period across a new year names both years", () => {
		expect(period("2025-12-15T00:00:00Z", "2026-01-15T00:00:00Z", UTC)).toBe(
			"Dec 15, 2025 to Jan 15, 2026",
		);
	});
});

describe("bands", () => {
	// Kills a band that starts at the last one's bound rather than past it, a
	// top band drawn with a bound, and the contact band given a price.
	test("each band runs from past the last one's bound, the top band open", () => {
		expect(bands(PRO)).toEqual([
			{ range: "0 to 250", price: "$100 a month" },
			{ range: "251 to 375", price: "$150 a month" },
			{ range: "376 to 500", price: "$200 a month" },
			{ range: "501 or more", price: undefined },
		]);
	});

	// Kills a per series price dropped, or a zero part drawn.
	test("a band's price adds its per series part and leaves out a zero", () => {
		expect(
			bands([
				{ up_to: 10, flat_amount: 0, unit_amount: 5 },
				{ up_to: 20, flat_amount: 1_000, unit_amount: 5 },
			]).map(({ price }) => price),
		).toEqual(["$0.05 a series", "$10 a month plus $0.05 a series"]);
	});
});

describe("bandIndex", () => {
	// Kills a bound read as exclusive, and series past every bound left without a band.
	test("a band holds its bound, and the open band holds the rest", () => {
		expect(bandIndex(PRO, 0)).toBe(0);
		expect(bandIndex(PRO, 250)).toBe(0);
		expect(bandIndex(PRO, 251)).toBe(1);
		expect(bandIndex(PRO, 9_999)).toBe(3);
	});
});

describe("estimate", () => {
	const plan = (status: string) => ({ status, tiers: PRO });

	// Kills an estimate that drops the band's fee or the bare metal hours.
	test("the band's fee plus bare metal at a dollar an hour", () => {
		expect(estimate(plan("active"), 212, 318)).toBeCloseTo(105.3);
		expect(estimate(plan("active"), 300, 0)).toBe(150);
	});

	// Kills a trial billed as a paid period.
	test("a trial costs nothing", () => {
		expect(estimate(plan("trialing"), 212, 318)).toBe(0);
	});

	// Kills an estimate drawn for usage past the self serve bands, or with no bands at all.
	test("past the bands, or without them, there is no estimate", () => {
		expect(estimate(plan("active"), 600, 0)).toBeUndefined();
		expect(estimate({ status: "active" }, 10, 0)).toBeUndefined();
	});
});

// Kills money drawn without its cents or its separators.
test("usd keeps cents and groups thousands", () => {
	expect(usd(5.3)).toBe("$5.30");
	expect(usd(1234)).toBe("$1,234.00");
});

// Kills the payer's name and email kept in the cache the browser stores.
test("usage keeps the plan without its payer", async () => {
	const plan = {
		level: "pro",
		customer: { name: "Muriel Bagge", email: "m@nowhere.example" },
	};
	const api = {
		get: async () => ({
			data: { kind: "cloud_metered", plan },
			headers: new Headers(),
		}),
	} as unknown as Api;
	const usage = await usageQuery(api, "o1").queryFn({
		signal: new AbortController().signal,
	});
	expect(usage.plan).toEqual({ level: "pro" });
});
