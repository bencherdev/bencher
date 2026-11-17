import { describe, expect, test } from "vitest";
import type { JsonPriceTier } from "../../types/bencher";
import type { Api } from "../api";
import {
	bandIndex,
	bands,
	bareMetalRate,
	bareMetalUsd,
	estimate,
	period,
	usageQuery,
	usd,
} from "./billing";

const UTC = "UTC";

// As the API sends them: an absent amount or bound is null, and the open band
// carries a placeholder price Stripe requires.
const PRO = JSON.parse(`[
	{ "up_to": 250, "unit_amount": null, "flat_amount": 10000 },
	{ "up_to": 375, "unit_amount": null, "flat_amount": 15000 },
	{ "up_to": 500, "unit_amount": null, "flat_amount": 20000 },
	{ "up_to": null, "unit_amount": null, "flat_amount": 50000 }
]`) as JsonPriceTier[];

// A deal of 1,000 series for $2,500 a month, then $0.40 a series.
const DEAL = JSON.parse(`[
	{ "up_to": 1000, "unit_amount": null, "flat_amount": 250000 },
	{ "up_to": null, "unit_amount": 40, "flat_amount": null }
]`) as JsonPriceTier[];

// Bare metal in cents a runner minute: the published dollar an hour, and a deal's two.
const DOLLAR_AN_HOUR = 100 / 60;
const TWO_DOLLARS_AN_HOUR = 200 / 60;

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
		expect(bands(PRO, "pro")).toEqual([
			{ range: "0 to 250", price: "$100 a month" },
			{ range: "251 to 375", price: "$150 a month" },
			{ range: "376 to 500", price: "$200 a month" },
			{ range: "501 or more", price: undefined },
		]);
	});

	// Kills a bound printed without its thousands grouped.
	test("groups a band's bounds", () => {
		expect(
			bands(
				[
					{ up_to: 250_000, flat_amount: 0, unit_amount: 5 },
					{ up_to: 1_000_000, flat_amount: 0, unit_amount: 5 },
				],
				"pro",
			).map(({ range }) => range),
		).toEqual(["0 to 250,000", "250,001 to 1,000,000"]);
	});

	// Kills a per series price dropped, or a zero part drawn.
	test("a band's price adds its per series part and leaves out a zero", () => {
		expect(
			bands(
				[
					{ up_to: 10, flat_amount: 0, unit_amount: 5 },
					{ up_to: 20, flat_amount: 1_000, unit_amount: 5 },
				],
				"pro",
			).map(({ price }) => price),
		).toEqual(["$0.05 a series", "$10 a month plus $0.05 a series"]);
	});

	// Kills an Enterprise plan's own overage band drawn as a conversation.
	test("an Enterprise plan prices its open band", () => {
		expect(bands(DEAL, "enterprise")).toEqual([
			{ range: "0 to 1,000", price: "$2,500 a month" },
			{ range: "1,001 or more", price: "$0.40 a series" },
		]);
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
	const plan = (status: string) => ({
		status,
		level: "pro",
		tiers: PRO,
		tiers_mode: "volume",
		runner_minute: DOLLAR_AN_HOUR,
	});
	const deal = {
		status: "active",
		level: "enterprise",
		tiers: DEAL,
		tiers_mode: "graduated",
		runner_minute: TWO_DOLLARS_AN_HOUR,
	};

	// Kills an estimate that drops the band's fee or the bare metal hours.
	test("the band's fee plus bare metal at the plan's rate", () => {
		expect(estimate(plan("active"), 212, 318)).toBeCloseTo(105.3);
		expect(estimate(plan("active"), 300, 0)).toBe(150);
	});

	// Kills a trial billed as a paid period.
	test("a trial costs nothing", () => {
		expect(estimate(plan("trialing"), 212, 318)).toBe(0);
	});

	// Kills an estimate drawn for usage past the self serve bands, or with no bands at all.
	test("past Pro's bands, or without bands, there is no estimate", () => {
		expect(estimate(plan("active"), 600, 0)).toBeUndefined();
		expect(estimate({ status: "active", level: "pro" }, 10, 0)).toBeUndefined();
	});

	// Kills a graduated deal billed as a volume price, its overage left out, or its fee
	// charged again above the commitment.
	test("a graduated deal charges its fee, then each series above it", () => {
		expect(estimate(deal, 812, 0)).toBe(2_500);
		expect(estimate(deal, 1_000, 0)).toBe(2_500);
		expect(estimate(deal, 1_200, 0)).toBeCloseTo(2_580);
		expect(estimate(deal, 1_200, 318)).toBeCloseTo(2_590.6);
	});

	// Kills an Enterprise volume price whose open band is left unpriced.
	test("an Enterprise volume price prices its open band", () => {
		expect(
			estimate(
				{
					status: "active",
					level: "enterprise",
					tiers_mode: "volume",
					tiers: JSON.parse(`[
						{ "up_to": 100, "unit_amount": null, "flat_amount": 1000 },
						{ "up_to": null, "unit_amount": 5, "flat_amount": null }
					]`) as JsonPriceTier[],
				},
				200,
				0,
			),
		).toBe(10);
	});

	// Kills bare metal billed at a rate the plan does not have.
	test("bare metal without a rate costs nothing", () => {
		expect(estimate({ ...plan("active"), runner_minute: null }, 212, 318)).toBe(
			100,
		);
	});
});

describe("bare metal", () => {
	// Kills bare metal priced at the published rate rather than the plan's own.
	test("costs and names the plan's own rate", () => {
		expect(bareMetalUsd(318, TWO_DOLLARS_AN_HOUR)).toBeCloseTo(10.6);
		expect(bareMetalRate(TWO_DOLLARS_AN_HOUR)).toBe("$2.00 an hour");
		expect(bareMetalRate(DOLLAR_AN_HOUR)).toBe("$1.00 an hour");
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
