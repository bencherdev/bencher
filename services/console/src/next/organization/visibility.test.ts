import { describe, expect, test } from "vitest";
import { type JsonUsage, PlanStatus, UsageKind } from "../../types/bencher";
import { allowsPrivate } from "./visibility";

const usage = (overrides: Partial<JsonUsage>): JsonUsage => ({
	organization: "o1",
	kind: UsageKind.CloudFree,
	start_time: "2026-09-01T00:00:00Z",
	end_time: "2026-10-01T00:00:00Z",
	...overrides,
});

const plan = (status: PlanStatus) =>
	({ status }) as unknown as NonNullable<JsonUsage["plan"]>;

const license = (series: number) =>
	({ series }) as unknown as NonNullable<JsonUsage["license"]>;

describe("allowsPrivate", () => {
	// Kills a metered plan that never allows private, and one that still does
	// once Stripe no longer counts it active.
	test("a Bencher Cloud plan allows private while it is active or trialing", () => {
		const metered = (status: PlanStatus) =>
			allowsPrivate(
				usage({ kind: UsageKind.CloudMetered, plan: plan(status) }),
			);
		expect(metered(PlanStatus.Active)).toBe(true);
		expect(metered(PlanStatus.Trialing)).toBe(true);
		expect(metered(PlanStatus.Canceled)).toBe(false);
		expect(metered(PlanStatus.PastDue)).toBe(false);
	});

	// Kills a license that allows private only under its Benchmark Series, or not at all:
	// the API accepts every report from a valid license and only warns over the count.
	test("a valid self-hosted license allows private, whatever its count", () => {
		const licensed = (active_series: number) =>
			allowsPrivate(
				usage({
					kind: UsageKind.SelfHostedLicensed,
					license: license(1000),
					active_series,
				}),
			);
		expect(licensed(999)).toBe(true);
		expect(licensed(1000)).toBe(true);
		expect(licensed(5000)).toBe(true);
	});

	// Kills a free organization offered private, and a Bencher Cloud license
	// for a self-hosted install read as this organization's own.
	test("no plan and no license allow no private project", () => {
		expect(allowsPrivate(usage({ kind: UsageKind.CloudFree }))).toBe(false);
		expect(allowsPrivate(usage({ kind: UsageKind.SelfHostedFree }))).toBe(
			false,
		);
		expect(
			allowsPrivate(
				usage({
					kind: UsageKind.CloudSelfHostedLicensed,
					license: license(1000),
				}),
			),
		).toBe(false);
	});
});
