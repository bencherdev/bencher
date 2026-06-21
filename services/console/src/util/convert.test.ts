import { describe, expect, test } from "vitest";
import { PlanLevel } from "../types/bencher";
import {
	addToArray,
	arrayFromString,
	arrayToString,
	base64ToBytes,
	bytesToBase64,
	dateTimeMillis,
	dateToTime,
	decodeBase64,
	encodeBase64,
	fmtDate,
	fmtUsd,
	fmtUsdPrecise,
	isBoolParam,
	isFirstBillingPeriod,
	planLevel,
	planLevelPrice,
	prettyPrintFloat,
	removeFromArray,
	runnerMinutePrice,
	sizeArray,
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
		expect(planLevel(PlanLevel.Team)).toBe("Team");
		expect(planLevel(PlanLevel.Pro)).toBe("Pro");
		expect(planLevel(PlanLevel.Enterprise)).toBe("Enterprise");
	});

	test("returns 'Bencher Plus' for undefined", () => {
		expect(planLevel(undefined)).toBe("Bencher Plus");
	});
});

describe("planLevelPrice", () => {
	test("returns correct price for each level", () => {
		expect(planLevelPrice(PlanLevel.Free)).toBe(0.0);
		expect(planLevelPrice(PlanLevel.Team)).toBe(0.01);
		expect(planLevelPrice(PlanLevel.Pro)).toBe(0.01);
		expect(planLevelPrice(PlanLevel.Enterprise)).toBe(0.05);
	});

	test("returns 0.0 for undefined", () => {
		expect(planLevelPrice(undefined)).toBe(0.0);
	});
});

describe("runnerMinutePrice", () => {
	test("returns price for paid plans", () => {
		expect(runnerMinutePrice(PlanLevel.Team)).toBe(0.01666);
		expect(runnerMinutePrice(PlanLevel.Pro)).toBe(0.01666);
		expect(runnerMinutePrice(PlanLevel.Enterprise)).toBe(0.01666);
	});

	test("returns 0.0 for free and undefined", () => {
		expect(runnerMinutePrice(PlanLevel.Free)).toBe(0.0);
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
