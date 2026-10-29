import type {
	JsonCardDetails,
	JsonPlan,
	JsonPriceTier,
	JsonUsage,
	PlanLevel,
} from "../../types/bencher";
import type { Api } from "../api";
import { formatCount } from "../plot/count";

const path = (organization: string) =>
	`/v0/organizations/${encodeURIComponent(organization)}`;

const LEVELS: Record<`${PlanLevel}`, true> = {
	free: true,
	pro: true,
	team: true,
	enterprise: true,
};

/** `level` if it names a plan, so text from a link never reaches the page. */
export const planLevel = (level: string | null): PlanLevel | undefined =>
	level !== null && Object.hasOwn(LEVELS, level)
		? (level as PlanLevel)
		: undefined;

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

/** "Oct 1 to Oct 31, 2026", with the start's year only when it differs. */
export const period = (start: string, end: string, timeZone?: string) => {
	const from = fullDay(start, timeZone);
	const to = fullDay(end, timeZone);
	return `${from.slice(-4) === to.slice(-4) ? shortDay(start, timeZone) : from} to ${to}`;
};

/** The top band has no bound; its price is a conversation. */
const isOpen = (tier: JsonPriceTier) => tier.up_to == null;

const bandPrice = (tier: JsonPriceTier) => {
	if (isOpen(tier)) {
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

export const bands = (tiers: JsonPriceTier[]) =>
	tiers.map((tier, index) => {
		const from = index === 0 ? 0 : (tiers[index - 1]?.up_to ?? 0) + 1;
		return {
			range: isOpen(tier)
				? `${formatCount(from)} or more`
				: `${formatCount(from)} to ${formatCount(tier.up_to ?? 0)}`,
			price: bandPrice(tier),
		};
	});

/** The band `series` active series fall in. */
export const bandIndex = (tiers: JsonPriceTier[], series: number) =>
	tiers.findIndex((tier) => isOpen(tier) || series <= (tier.up_to ?? 0));

/** This period's bill in dollars, unless the usage is past the self serve bands. */
export const estimate = (
	plan: { status: string; tiers?: JsonPriceTier[] },
	series: number,
	minutes: number,
) => {
	if (plan.status === "trialing") {
		return 0;
	}
	if (!plan.tiers) {
		return undefined;
	}
	const tier = plan.tiers[bandIndex(plan.tiers, series)];
	if (!tier || isOpen(tier)) {
		return undefined;
	}
	return (
		((tier.flat_amount ?? 0) + (tier.unit_amount ?? 0) * series) / 100 +
		bareMetalUsd(minutes)
	);
};

export const bareMetalUsd = (minutes: number) =>
	(minutes / 60) * BARE_METAL_HOURLY_USD;

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
