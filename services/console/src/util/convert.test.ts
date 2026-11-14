import { describe, expect, test } from "vitest";
import { type JsonPriceTier, PlanLevel } from "../types/bencher";
import {
	addToArray,
	arrayFromString,
	arrayToString,
	base64ToBytes,
	bytesToBase64,
	currentSeriesTier,
	currentSeriesTierIndex,
	dateTimeMillis,
	dateToTime,
	decodeBase64,
	encodeBase64,
	fmtDate,
	fmtTierPrice,
	fmtUsd,
	fmtUsdPrecise,
	isBoolParam,
	isContactTier,
	isFirstBillingPeriod,
	planLevel,
	prettyPrintFloat,
	removeFromArray,
	planEstimateUsd,
	runnerMinutePrice,
	seriesEstimateUsd,
	seriesTierRange,
	sizeArray,
	tierFlatUsd,
	tierUnitUsd,
	timeToDate,
	timeToDateIso,
	timeToDateOnlyIso,
} from "./convert";

describe("dateTimeMillis", () => {
	test("returns milliseconds for valid ISO string", () => {
		const result = dateTimeMillis("2024-01-15T12:00:00Z");
		expect(result).toBe(Date.parse("2024-01-15T12:00:00Z"));
	});

	test("returns null for undefined", () => {
		expect(dateTimeMillis(undefined)).toBeNull();
	});

	test("returns null for invalid date string", () => {
		expect(dateTimeMillis("not-a-date")).toBeNull();
	});
});

describe("fmtDate", () => {
	test("returns formatted date string for valid input", () => {
		const result = fmtDate("2024-01-15T12:00:00Z");
		expect(result).toBe(new Date("2024-01-15T12:00:00Z").toDateString());
	});

	test("returns null for undefined", () => {
		expect(fmtDate(undefined)).toBeNull();
	});

	test("returns null for invalid date string", () => {
		expect(fmtDate("not-a-date")).toBeNull();
	});
});

describe("addToArray", () => {
	test("adds new element and returns its index", () => {
		const [arr, idx] = addToArray(["a", "b"], "c");
		expect(arr).toEqual(["a", "b", "c"]);
		expect(idx).toBe(2);
	});

	test("returns null index for duplicate", () => {
		const [arr, idx] = addToArray(["a", "b"], "a");
		expect(arr).toEqual(["a", "b"]);
		expect(idx).toBeNull();
	});

	test("adds to empty array", () => {
		const [arr, idx] = addToArray([], "x");
		expect(arr).toEqual(["x"]);
		expect(idx).toBe(0);
	});
});

describe("removeFromArray", () => {
	test("removes existing element and returns its index", () => {
		const [arr, idx] = removeFromArray(["a", "b", "c"], "b");
		expect(arr).toEqual(["a", "c"]);
		expect(idx).toBe(1);
	});

	test("returns null index for missing element", () => {
		const [arr, idx] = removeFromArray(["a", "b"], "z");
		expect(arr).toEqual(["a", "b"]);
		expect(idx).toBeNull();
	});

	test("removes from single-element array", () => {
		const [arr, idx] = removeFromArray(["a"], "a");
		expect(arr).toEqual([]);
		expect(idx).toBe(0);
	});
});

describe("arrayFromString", () => {
	test("splits comma-separated string", () => {
		expect(arrayFromString("a,b,c")).toEqual(["a", "b", "c"]);
	});

	test("returns empty array for empty string", () => {
		expect(arrayFromString("")).toEqual([]);
	});

	test("returns empty array for undefined", () => {
		expect(arrayFromString(undefined)).toEqual([]);
	});

	test("returns single-element array for string without commas", () => {
		expect(arrayFromString("abc")).toEqual(["abc"]);
	});
});

describe("arrayToString", () => {
	test("joins array with commas", () => {
		expect(arrayToString(["a", "b", "c"])).toBe("a,b,c");
	});

	test("returns empty string for empty array", () => {
		expect(arrayToString([])).toBe("");
	});
});

describe("sizeArray", () => {
	test("pads child to parent length with empty strings", () => {
		expect(sizeArray(["a", "b", "c"], ["x"])).toEqual(["x", "", ""]);
	});

	test("truncates child to parent length", () => {
		expect(sizeArray(["a"], ["x", "y", "z"])).toEqual(["x"]);
	});

	test("coalesces null values to empty string", () => {
		expect(sizeArray(["a", "b"], [null, "y"])).toEqual(["", "y"]);
	});
});

describe("timeToDate", () => {
	test("converts timestamp string to Date", () => {
		const ts = "1705312800000";
		const result = timeToDate(ts);
		expect(result).toBeInstanceOf(Date);
		expect(result?.getTime()).toBe(1705312800000);
	});

	test("returns null for undefined", () => {
		expect(timeToDate(undefined)).toBeNull();
	});

	test("returns null for non-integer string", () => {
		expect(timeToDate("abc")).toBeNull();
	});
});

describe("timeToDateIso", () => {
	test("converts timestamp to ISO string", () => {
		const result = timeToDateIso("0");
		expect(result).toBe("1970-01-01T00:00:00.000Z");
	});

	test("returns null for undefined", () => {
		expect(timeToDateIso(undefined)).toBeNull();
	});
});

describe("timeToDateOnlyIso", () => {
	test("returns date-only ISO string", () => {
		const result = timeToDateOnlyIso("0");
		expect(result).toBe("1970-01-01");
	});

	test("returns undefined for undefined input", () => {
		expect(timeToDateOnlyIso(undefined)).toBeUndefined();
	});
});

describe("dateToTime", () => {
	test("converts date string to timestamp string", () => {
		const result = dateToTime("2024-01-15T12:00:00Z");
		expect(result).toBe(`${Date.parse("2024-01-15T12:00:00Z")}`);
	});

	test("returns null for undefined", () => {
		expect(dateToTime(undefined)).toBeNull();
	});

	test("returns null for invalid date", () => {
		expect(dateToTime("not-a-date")).toBeNull();
	});
});

describe("isBoolParam", () => {
	test("returns true for 'true'", () => {
		expect(isBoolParam("true")).toBe(true);
	});

	test("returns true for 'false'", () => {
		expect(isBoolParam("false")).toBe(true);
	});

	test("returns false for undefined", () => {
		expect(isBoolParam(undefined)).toBe(false);
	});

	test("returns false for other strings", () => {
		expect(isBoolParam("yes")).toBe(false);
		expect(isBoolParam("1")).toBe(false);
		expect(isBoolParam("")).toBe(false);
	});
});

describe("planLevel", () => {
	test("returns correct name for each level", () => {
		expect(planLevel(PlanLevel.Free)).toBe("Free");
		expect(planLevel(PlanLevel.Pro)).toBe("Pro");
		expect(planLevel(PlanLevel.Enterprise)).toBe("Enterprise");
	});

	test("returns 'Bencher Plus' for undefined", () => {
		expect(planLevel(undefined)).toBe("Bencher Plus");
	});
});

describe("runnerMinutePrice", () => {
	// Kills bare metal priced at the published rate rather than the plan's own.
	test("returns the plan's own rate in dollars a minute", () => {
		expect(runnerMinutePrice({ runner_minute: 100 / 60 })).toBeCloseTo(
			0.016_666_67,
		);
		expect(runnerMinutePrice({ runner_minute: 200 / 60 })).toBeCloseTo(
			0.033_333_33,
		);
	});

	test("returns 0.0 for a plan without a rate, or no plan", () => {
		expect(runnerMinutePrice({ runner_minute: null })).toBe(0.0);
		expect(runnerMinutePrice(undefined)).toBe(0.0);
	});
});

describe("isFirstBillingPeriod", () => {
	test("true when created equals the current period start", () => {
		expect(
			isFirstBillingPeriod("2026-06-01T00:00:00Z", "2026-06-01T00:00:00Z"),
		).toBe(true);
	});

	test("true when created is within the current period", () => {
		expect(
			isFirstBillingPeriod("2026-06-01T00:00:05Z", "2026-06-01T00:00:00Z"),
		).toBe(true);
	});

	test("false for a later period (created before the period start)", () => {
		expect(
			isFirstBillingPeriod("2026-06-01T00:00:00Z", "2026-07-01T00:00:00Z"),
		).toBe(false);
	});

	test("false when either timestamp is undefined", () => {
		expect(isFirstBillingPeriod(undefined, "2026-06-01T00:00:00Z")).toBe(false);
		expect(isFirstBillingPeriod("2026-06-01T00:00:00Z", undefined)).toBe(false);
	});
});

describe("fmtUsd", () => {
	test("formats positive amount", () => {
		expect(fmtUsd(10.5)).toBe("$10.50");
	});

	test("formats zero for undefined", () => {
		expect(fmtUsd(undefined)).toBe("$0.00");
	});

	test("formats zero", () => {
		expect(fmtUsd(0)).toBe("$0.00");
	});
});

describe("fmtUsdPrecise", () => {
	test("formats with 5 decimal places", () => {
		expect(fmtUsdPrecise(0.01666)).toBe("$0.01666");
	});

	test("formats zero for undefined", () => {
		expect(fmtUsdPrecise(undefined)).toBe("$0.00000");
	});
});

describe("base64 round-trip", () => {
	test("encodeBase64 and decodeBase64 are inverses", () => {
		const input = "Hello, World!";
		expect(decodeBase64(encodeBase64(input))).toBe(input);
	});

	test("handles unicode text", () => {
		const input = "Unicode: éèê";
		expect(decodeBase64(encodeBase64(input))).toBe(input);
	});

	test("base64ToBytes returns Uint8Array", () => {
		const bytes = base64ToBytes(btoa("abc"));
		expect(bytes).toBeInstanceOf(Uint8Array);
		expect(bytes.length).toBe(3);
	});

	test("bytesToBase64 encodes bytes", () => {
		const bytes = new TextEncoder().encode("test");
		const b64 = bytesToBase64(bytes);
		expect(atob(b64)).toBe("test");
	});
});

describe("prettyPrintFloat", () => {
	test("formats float with 2 decimal places", () => {
		expect(prettyPrintFloat(1234.5)).toBe("1,234.50");
	});

	test("returns undefined for undefined", () => {
		expect(prettyPrintFloat(undefined)).toBeUndefined();
	});

	test("formats zero", () => {
		expect(prettyPrintFloat(0)).toBe("0.00");
	});
});

// Build a tier with the real wire shape. The API sends JSON `null` for an absent Option
// field, though the generated `JsonPriceTier` models these as optional; the cast lets
// fixtures use `null` so the helpers are exercised against production data, not just
// omitted keys (which was how a `null`-vs-`undefined` bug slipped through before).
const tier = (
	up_to: number | null,
	flat_amount: number | null,
	unit_amount: number | null,
): JsonPriceTier =>
	({ up_to, flat_amount, unit_amount }) as unknown as JsonPriceTier;

// A deal of 1,000 series for $2,500 a month, then $0.40 a series.
const DEAL_TIERS: JsonPriceTier[] = [
	tier(1_000, 250_000, null),
	tier(null, null, 40),
];

// A representative Pro ladder: 0-250 $100, 251-375 $150, 376-500 $200, 501+ Get in Touch.
// Every bounded tier leaves `unit_amount` null and the unbounded top tier carries a Stripe
// placeholder ($0.40/series), exactly as the live API sends them.
const SERIES_TIERS: JsonPriceTier[] = [
	tier(250, 10_000, null),
	tier(375, 15_000, null),
	tier(500, 20_000, null),
	tier(null, 0, 40),
];

describe("currentSeriesTier", () => {
	test("selects the band containing the series count", () => {
		expect(currentSeriesTier(SERIES_TIERS, 0)?.up_to).toBe(250);
		expect(currentSeriesTier(SERIES_TIERS, 250)?.up_to).toBe(250);
		expect(currentSeriesTier(SERIES_TIERS, 251)?.up_to).toBe(375);
		expect(currentSeriesTier(SERIES_TIERS, 375)?.up_to).toBe(375);
		expect(currentSeriesTier(SERIES_TIERS, 376)?.up_to).toBe(500);
		expect(currentSeriesTier(SERIES_TIERS, 500)?.up_to).toBe(500);
	});

	test("returns the unbounded contact tier above the last bound", () => {
		expect(
			isContactTier(currentSeriesTier(SERIES_TIERS, 501), PlanLevel.Pro),
		).toBe(true);
		expect(
			isContactTier(currentSeriesTier(SERIES_TIERS, 10_000), PlanLevel.Pro),
		).toBe(true);
	});

	test("returns undefined when tiers are absent", () => {
		expect(currentSeriesTier(undefined, 100)).toBeUndefined();
	});
});

describe("currentSeriesTierIndex", () => {
	test("selects the index of the band containing the series count", () => {
		expect(currentSeriesTierIndex(SERIES_TIERS, 0)).toBe(0);
		expect(currentSeriesTierIndex(SERIES_TIERS, 250)).toBe(0);
		expect(currentSeriesTierIndex(SERIES_TIERS, 251)).toBe(1);
		expect(currentSeriesTierIndex(SERIES_TIERS, 375)).toBe(1);
		expect(currentSeriesTierIndex(SERIES_TIERS, 376)).toBe(2);
		expect(currentSeriesTierIndex(SERIES_TIERS, 500)).toBe(2);
	});

	test("returns the unbounded contact tier index above the last bound", () => {
		expect(currentSeriesTierIndex(SERIES_TIERS, 501)).toBe(3);
		expect(currentSeriesTierIndex(SERIES_TIERS, 10_000)).toBe(3);
	});

	test("returns -1 when tiers are absent", () => {
		expect(currentSeriesTierIndex(undefined, 100)).toBe(-1);
	});
});

describe("seriesTierRange", () => {
	test("formats inclusive ranges from tier bounds", () => {
		expect(seriesTierRange(SERIES_TIERS, 0)).toBe("0-250");
		expect(seriesTierRange(SERIES_TIERS, 1)).toBe("251-375");
		expect(seriesTierRange(SERIES_TIERS, 2)).toBe("376-500");
	});

	test("formats the unbounded (null `up_to`) top tier with a plus", () => {
		expect(seriesTierRange(SERIES_TIERS, 3)).toBe("501+");
	});
});

describe("isContactTier", () => {
	test("true for Pro's unbounded top tier, including a null `up_to`", () => {
		expect(isContactTier(SERIES_TIERS[3], PlanLevel.Pro)).toBe(true);
		expect(isContactTier({}, PlanLevel.Pro)).toBe(true);
		expect(isContactTier(undefined, PlanLevel.Pro)).toBe(true);
	});

	test("false for a bounded tier", () => {
		expect(isContactTier(SERIES_TIERS[0], PlanLevel.Pro)).toBe(false);
	});

	// Kills an Enterprise plan's own overage band drawn as a conversation.
	test("false for an Enterprise plan's open tier", () => {
		expect(isContactTier(DEAL_TIERS[1], PlanLevel.Enterprise)).toBe(false);
	});
});

describe("tierFlatUsd / tierUnitUsd", () => {
	test("convert cents to USD", () => {
		expect(tierFlatUsd(SERIES_TIERS[0])).toBe(100);
		expect(tierUnitUsd(SERIES_TIERS[3])).toBe(0.4);
	});

	test("null when the amount is null or absent", () => {
		expect(tierUnitUsd(SERIES_TIERS[0])).toBeNull();
		expect(tierFlatUsd({ up_to: 250 })).toBeNull();
	});
});

describe("fmtTierPrice", () => {
	const pro = PlanLevel.Pro;

	test("flat-only tier omits an absent per-series fee", () => {
		expect(fmtTierPrice(SERIES_TIERS[0], pro)).toBe("$100.00 / month");
	});

	test("drops a $0.00 component rather than showing it", () => {
		expect(fmtTierPrice(tier(250, 10_000, null), pro)).toBe("$100.00 / month");
		expect(fmtTierPrice(tier(250, 10_000, 0), pro)).toBe("$100.00 / month");
	});

	test("per-series-only tier", () => {
		expect(fmtTierPrice(tier(250, null, 5), pro)).toBe("$0.05 / series");
	});

	test("tier with both a flat fee and a per-series fee", () => {
		expect(fmtTierPrice(tier(250, 10_000, 5), pro)).toBe(
			"$100.00 / month + $0.05 / series",
		);
	});

	test("Pro's unbounded tier is contact sales even with a placeholder price", () => {
		expect(fmtTierPrice(SERIES_TIERS[3], pro)).toBe("Get in Touch");
		expect(fmtTierPrice({}, pro)).toBe("Get in Touch");
		expect(fmtTierPrice(undefined, pro)).toBe("Get in Touch");
	});

	// Kills an Enterprise plan's own overage band drawn as a conversation.
	test("an Enterprise plan's unbounded tier shows its price", () => {
		expect(fmtTierPrice(DEAL_TIERS[1], PlanLevel.Enterprise)).toBe(
			"$0.40 / series",
		);
	});

	// Kills a band without a price drawn as a conversation on a plan that is not Pro.
	test("an Enterprise tier without a price shows a dash", () => {
		expect(fmtTierPrice({}, PlanLevel.Enterprise)).toBe("-");
		expect(fmtTierPrice(tier(1_000, 0, null), PlanLevel.Enterprise)).toBe("-");
	});
});

describe("seriesEstimateUsd", () => {
	const pro = (tiers: JsonPriceTier[]) => ({
		level: PlanLevel.Pro,
		tiers,
		tiers_mode: "volume",
	});
	const deal = {
		level: PlanLevel.Enterprise,
		tiers: DEAL_TIERS,
		tiers_mode: "graduated",
	};

	test("flat-only tier ignores the series count", () => {
		expect(seriesEstimateUsd(pro([tier(250, 10_000, null)]), 200)).toBe(100);
	});

	test("adds the per-series fee times the series count", () => {
		expect(seriesEstimateUsd(pro([tier(500, 10_000, 5)]), 300)).toBe(
			100 + 0.05 * 300,
		);
	});

	test("returns null for Pro's unbounded contact tier, or no tiers", () => {
		expect(seriesEstimateUsd(pro(SERIES_TIERS), 600)).toBeNull();
		expect(seriesEstimateUsd({ level: PlanLevel.Pro }, 600)).toBeNull();
	});

	// Kills a graduated deal billed as a volume price, or its overage left out.
	test("a graduated deal charges its fee, then each series above it", () => {
		expect(seriesEstimateUsd(deal, 812)).toBe(2_500);
		expect(seriesEstimateUsd(deal, 1_200)).toBeCloseTo(2_580);
	});
});

describe("planEstimateUsd", () => {
	// Kills a total drawn from the runner cost alone for a price the series estimate
	// cannot read, or one that drops the series charge or the plan's own bare metal rate.
	test("adds the plan's bare metal, or is null without a series estimate", () => {
		const deal = {
			level: PlanLevel.Enterprise,
			tiers: DEAL_TIERS,
			tiers_mode: "graduated",
			runner_minute: 200 / 60,
		};
		expect(planEstimateUsd(deal, 1_200, 318)).toBeCloseTo(2_590.6);
		expect(
			planEstimateUsd(
				{ level: PlanLevel.Enterprise, tiers: null, runner_minute: 200 / 60 },
				1_200,
				318,
			),
		).toBeNull();
	});
});
