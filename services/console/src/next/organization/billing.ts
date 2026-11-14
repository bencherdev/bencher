import type {
	JsonCardDetails,
	JsonLicense,
	JsonPlan,
	JsonPriceTier,
	JsonUsage,
} from "../../types/bencher";
import type { Api } from "../api";
import { formatCount } from "../plot/count";

const path = (organization: string) =>
	`/v0/organizations/${encodeURIComponent(organization)}`;

/** A plan as the page keeps it: without the payer's name and email, which it never shows. */
export type Plan = Omit<JsonPlan, "customer">;
export type Usage = Omit<JsonUsage, "plan"> & { plan?: Plan };

export const withoutPayer = ({ customer: _, ...plan }: JsonPlan): Plan => plan;

export const usageQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization, "usage"],
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<Usage> => {
		const { data } = await api.get<JsonUsage>(
			`${path(organization)}/usage`,
			signal,
		);
		return data.plan ? { ...data, plan: withoutPayer(data.plan) } : data;
	},
	// A return from Stripe paints what the cache holds, then the plan as it is now.
	staleTime: 0,
});

/** Whether the reader manages the organization, which every billing change needs. */
export const manageQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization, "allowed", "manage"],
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<{ allowed: boolean }>(
				`${path(organization)}/allowed/manage`,
				signal,
			)
		).data.allowed,
});

/** Where the page leaves for Stripe. */
export const away = { to: (url: string) => location.assign(url) };

const BARE_METAL_HOURLY_USD = 1;

const DOLLARS = new Intl.NumberFormat("en-US", {
	style: "currency",
	currency: "USD",
});

export const usd = (dollars: number) => DOLLARS.format(dollars);

const price = (cents: number) => usd(cents / 100).replace(/\.00$/, "");

const day = (time: string, timeZone: string | undefined, year: boolean) =>
	new Intl.DateTimeFormat("en-US", {
		month: "short",
		day: "numeric",
		...(year ? { year: "numeric" } : {}),
		...(timeZone ? { timeZone } : {}),
	}).format(Date.parse(time));

/** "Oct 31, 2026", in the reader's time zone unless one is given. */
export const fullDay = (time: string, timeZone?: string) =>
	day(time, timeZone, true);

/** "Oct 31", in the reader's time zone unless one is given. */
export const shortDay = (time: string, timeZone?: string) =>
	day(time, timeZone, false);

/** Whether a license is past the end of its paid term, in its grace, at the server's time `now`. */
export const inGrace = (license: JsonLicense, now: string) =>
	Date.parse(license.term_end) <= Date.parse(now);

/** Whether a license is at or past its expiration, at the server's time `now`. */
export const expired = (license: JsonLicense, now: string) =>
	Date.parse(license.expiration) <= Date.parse(now);

/** "Oct 1 to Oct 31, 2026", with the start's year only when it differs. */
export const period = (start: string, end: string, timeZone?: string) => {
	const from = fullDay(start, timeZone);
	const to = fullDay(end, timeZone);
	return `${from.slice(-4) === to.slice(-4) ? shortDay(start, timeZone) : from} to ${to}`;
};

/** The top band has no bound. */
const isOpen = (tier: JsonPriceTier) => tier.up_to == null;

/** Pro's top band is a conversation; an Enterprise deal's top band has its own price. */
const isContact = (tier: JsonPriceTier, level: string) =>
	isOpen(tier) && level === "pro";

const bandPrice = (tier: JsonPriceTier, level: string) => {
	if (isContact(tier, level)) {
		return undefined;
	}
	const parts: string[] = [];
	if (tier.flat_amount) {
		parts.push(`${price(tier.flat_amount)} a month`);
	}
	if (tier.unit_amount) {
		parts.push(`${price(tier.unit_amount)} a series`);
	}
	return parts.join(" plus ") || undefined;
};

export const bands = (tiers: JsonPriceTier[], level: string) =>
	tiers.map((tier, index) => {
		const from = index === 0 ? 0 : (tiers[index - 1]?.up_to ?? 0) + 1;
		return {
			range: isOpen(tier)
				? `${formatCount(from)} or more`
				: `${formatCount(from)} to ${formatCount(tier.up_to ?? 0)}`,
			price: bandPrice(tier, level),
		};
	});

/** The band `series` active series fall in. */
export const bandIndex = (tiers: JsonPriceTier[], series: number) =>
	tiers.findIndex((tier) => isOpen(tier) || series <= (tier.up_to ?? 0));

/** A volume price charges every series at the rate of the band the count falls in. */
const volumeCents = (tiers: JsonPriceTier[], level: string, series: number) => {
	const tier = tiers[bandIndex(tiers, series)];
	if (!tier || isContact(tier, level)) {
		return undefined;
	}
	return (tier.flat_amount ?? 0) + (tier.unit_amount ?? 0) * series;
};

/** A graduated price charges each band the count reaches for the series inside it. */
const graduatedCents = (tiers: JsonPriceTier[], series: number) =>
	tiers.reduce((cents, tier, index) => {
		const from = index === 0 ? 0 : (tiers[index - 1]?.up_to ?? 0);
		if (index > 0 && series <= from) {
			return cents;
		}
		const inside = Math.min(series, tier.up_to ?? series) - from;
		return cents + (tier.flat_amount ?? 0) + (tier.unit_amount ?? 0) * inside;
	}, 0);

/** This period's bill in dollars, unless the usage is past Pro's self serve bands. */
export const estimate = (
	plan: {
		status: string;
		level?: string;
		tiers?: JsonPriceTier[] | null;
		tiers_mode?: string | null;
		runner_minute?: number | null;
	},
	series: number,
	minutes: number,
) => {
	if (plan.status === "trialing") {
		return 0;
	}
	if (!plan.tiers) {
		return undefined;
	}
	const cents =
		plan.tiers_mode === "graduated"
			? graduatedCents(plan.tiers, series)
			: volumeCents(plan.tiers, plan.level ?? "", series);
	if (cents === undefined) {
		return undefined;
	}
	return cents / 100 + bareMetalUsd(minutes, plan.runner_minute);
};

/** The bare metal cost in dollars, at a rate in cents a runner minute. */
export const bareMetalUsd = (
	minutes: number,
	centsPerMinute: number | null | undefined,
) => (minutes * (centsPerMinute ?? 0)) / 100;

/** A rate in cents a runner minute, named by the hour. */
export const bareMetalRate = (centsPerMinute: number) =>
	`${usd((centsPerMinute * 60) / 100)} an hour`;

/** The published bare metal rate that a Pro plan starts with. */
export const BARE_METAL_RATE = `${usd(BARE_METAL_HOURLY_USD)} an hour`;

const BRANDS: Record<string, string> = {
	amex: "American Express",
	diners: "Diners Club",
	discover: "Discover",
	jcb: "JCB",
	mastercard: "Mastercard",
	unionpay: "UnionPay",
	visa: "Visa",
};

export const cardName = (card: JsonCardDetails) =>
	`${BRANDS[card.brand] ?? "Card"} ending ${card.last_four}`;

export const cardExpiry = (card: JsonCardDetails) =>
	`Expires ${String(card.exp_month).padStart(2, "0")}/${card.exp_year}`;
