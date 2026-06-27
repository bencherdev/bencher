import { type JsonPriceTier, ModelTest, PlanLevel } from "../types/bencher";

export const dateTimeMillis = (date_str: undefined | string) => {
	if (date_str === undefined) {
		return null;
	}
	const date_ms = Date.parse(date_str);
	if (date_ms) {
		const date = new Date(date_ms);
		if (date) {
			return date.getTime();
		}
	}
	return null;
};

export const fmtDate = (date_str: undefined | string) => {
	if (date_str === undefined) {
		return null;
	}
	const date_ms = Date.parse(date_str);
	if (date_ms) {
		const date = new Date(date_ms);
		if (date) {
			return date.toDateString();
		}
	}
	return null;
};

export const addToArray = (
	array: string[],
	add: string,
): [string[], null | number] => {
	if (array.includes(add)) {
		return [array, null];
	}
	const len = array.push(add);
	return [array, len - 1];
};
export const removeFromArray = (
	array: string[],
	remove: string,
): [string[], null | number] => {
	const index = array.indexOf(remove);
	if (index < 0) {
		return [array, null];
	}
	array.splice(index, 1);
	return [array, index];
};

export const arrayFromString = (array_str: undefined | string): string[] => {
	if (typeof array_str === "string") {
		if (array_str === "") {
			return [];
		}
		return array_str.split(",");
	}
	return [];
};
export const arrayToString = (array: string[]) => array.join();

export const sizeArray = (
	parent: string[],
	child: (string | null)[],
): (string | null)[] => parent.map((_, i) => child[i] ?? "");

export const timeToDate = (time_str: undefined | string): null | Date => {
	if (typeof time_str === "string") {
		const time = Number.parseInt(time_str);
		if (Number.isInteger(time)) {
			const date = new Date(time);
			if (date) {
				return date;
			}
		}
	}
	return null;
};

export const timeToDateIso = (time_str: undefined | string): null | string => {
	const date = timeToDate(time_str);
	if (date) {
		return date.toISOString();
	}
	return null;
};

export const timeToDateOnlyIso = (
	time_str: undefined | string,
): undefined | string => {
	const iso_date = timeToDateIso(time_str);
	if (iso_date) {
		return iso_date.split("T")?.[0];
	}
	return;
};

export const dateToTime = (date_str: undefined | string): null | string => {
	if (typeof date_str === "string") {
		const time = Date.parse(date_str);
		if (time) {
			return `${time}`;
		}
	}
	return null;
};

export const isBoolParam = (param: undefined | string): boolean => {
	return param === "false" || param === "true";
};

// The first billing period is the one containing the subscription's creation time,
// i.e. created falls on/after the current period start.
export const isFirstBillingPeriod = (
	created: undefined | string,
	currentPeriodStart: undefined | string,
): boolean => {
	const createdMs = dateTimeMillis(created);
	const periodStartMs = dateTimeMillis(currentPeriodStart);
	return (
		createdMs !== null && periodStartMs !== null && createdMs >= periodStartMs
	);
};

export const planLevel = (level: undefined | PlanLevel) => {
	switch (level) {
		case PlanLevel.Free:
			return "Free";
		case PlanLevel.Team:
			return "Team";
		case PlanLevel.Pro:
			return "Pro";
		case PlanLevel.Enterprise:
			return "Enterprise";
		default:
			return "Bencher Plus";
	}
};

export const planLevelPrice = (level: undefined | PlanLevel) => {
	switch (level) {
		case PlanLevel.Free:
			return 0.0;
		case PlanLevel.Team:
		case PlanLevel.Pro:
			return 0.01;
		case PlanLevel.Enterprise:
			return 0.05;
		default:
			return 0.0;
	}
};

export const runnerMinutePrice = (level: undefined | PlanLevel) => {
	switch (level) {
		case PlanLevel.Team:
		case PlanLevel.Pro:
		case PlanLevel.Enterprise:
			return 0.01666;
		default:
			return 0.0;
	}
};

export const fmtUsd = (usd: undefined | number) => {
	const numberFmt = new Intl.NumberFormat("en-US", {
		style: "currency",
		currency: "USD",
	});
	return numberFmt.format(usd ?? 0);
};

export const fmtUsdPrecise = (usd: undefined | number) => {
	const numberFmt = new Intl.NumberFormat("en-US", {
		style: "currency",
		currency: "USD",
		minimumFractionDigits: 5,
		maximumFractionDigits: 5,
	});
	return numberFmt.format(usd ?? 0);
};

// https://developer.mozilla.org/en-US/docs/Glossary/Base64#the_unicode_problem
export const base64ToBytes = (base64) => {
	const binString = atob(base64);
	return Uint8Array.from(binString, (m) => m.codePointAt(0));
};

export const decodeBase64 = (base64) =>
	new TextDecoder().decode(base64ToBytes(base64));

export const bytesToBase64 = (bytes) => {
	const binString = String.fromCodePoint(...bytes);
	return btoa(binString);
};

export const encodeBase64 = (str) =>
	bytesToBase64(new TextEncoder().encode(str));

export const prettyPrintFloat = (float: number | undefined) => {
	return float?.toLocaleString("en-US", {
		minimumFractionDigits: 2,
		maximumFractionDigits: 2,
	});
};

// Pro is billed on monthly-active series via a Stripe tiered price. The Console renders
// the ladder from `usage.plan.tiers` (the billed source of truth) instead of hardcoding
// it. A tier may carry a flat fee, a per-series fee, or both (additive). The unbounded top
// tier (no `up_to`) is the "Get in Touch" band; its Stripe price is only a required
// placeholder, so it is never surfaced as a self-serve rate.
//
// The API sends JSON `null` for an absent `Option` field, while the typeshare-generated
// type models it as optional. The runtime value of these fields is therefore `null`, not
// `undefined`, so the helpers below use `== null` / `??` to handle both forms.

const tierCentsToUsd = (cents: number | null | undefined): number | null =>
	cents == null ? null : cents / 100;

// The flat monthly fee (USD) for a tier, or null if it has none.
export const tierFlatUsd = (tier: JsonPriceTier | undefined): number | null =>
	tierCentsToUsd(tier?.flat_amount);

// The per-series fee (USD) for a tier, or null if it has none.
export const tierUnitUsd = (tier: JsonPriceTier | undefined): number | null =>
	tierCentsToUsd(tier?.unit_amount);

// The unbounded top tier (no `up_to`) is presented as "Get in Touch" (enterprise/sales),
// regardless of any placeholder price Stripe requires on it.
export const isContactTier = (tier: JsonPriceTier | undefined): boolean =>
	tier?.up_to == null;

// The index in `tiers` of the band containing `series`: the first tier with no upper
// bound or whose `up_to` is >= series. Tiers are ordered ascending by `up_to`. -1 when
// tiers are absent or no band matches.
export const currentSeriesTierIndex = (
	tiers: JsonPriceTier[] | undefined,
	series: number,
): number =>
	tiers?.findIndex((tier) => tier.up_to == null || series <= tier.up_to) ?? -1;

// The tier whose band contains `series` (see `currentSeriesTierIndex`).
export const currentSeriesTier = (
	tiers: JsonPriceTier[] | undefined,
	series: number,
): JsonPriceTier | undefined => tiers?.[currentSeriesTierIndex(tiers, series)];

// The inclusive series range label for the tier at `index`, e.g. "0-250", "251-375",
// "501+". The lower bound is the previous tier's `up_to` + 1 (0 for the first tier).
export const seriesTierRange = (
	tiers: JsonPriceTier[],
	index: number,
): string => {
	const lower = index === 0 ? 0 : (tiers[index - 1]?.up_to ?? 0) + 1;
	const upTo = tiers[index]?.up_to;
	return upTo == null ? `${lower}+` : `${lower}-${upTo}`;
};

// A human label for a tier's price: "Get in Touch" for the contact tier, otherwise the
// nonzero components joined, e.g. "$100.00 / month", "$0.05 / series", or
// "$100.00 / month + $0.05 / series".
export const fmtTierPrice = (tier: JsonPriceTier | undefined): string => {
	if (isContactTier(tier)) {
		return "Get in Touch";
	}
	// Only positive components are shown: a $0.00 flat or per-series amount (an absent or
	// null field, or a Stripe placeholder of 0) is noise, not a real charge.
	const parts: string[] = [];
	const flat = tierFlatUsd(tier);
	if (flat != null && flat > 0) {
		parts.push(`${fmtUsd(flat)} / month`);
	}
	const unit = tierUnitUsd(tier);
	if (unit != null && unit > 0) {
		parts.push(`${fmtUsd(unit)} / series`);
	}
	return parts.length > 0 ? parts.join(" + ") : "Get in Touch";
};

// The estimated monthly charge (USD) for being in `tier` with `series` active series:
// the flat fee plus the per-series fee times `series`. null for the contact tier (no
// self-serve price). The per-series component is forward-compatible; today only the flat
// fee is set.
export const tierEstimateUsd = (
	tier: JsonPriceTier | undefined,
	series: number,
): number | null => {
	if (isContactTier(tier)) {
		return null;
	}
	return (tierFlatUsd(tier) ?? 0) + (tierUnitUsd(tier) ?? 0) * series;
};
