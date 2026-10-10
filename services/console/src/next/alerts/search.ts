import type { JsonAlertsFilter } from "../../types/bencher";
import {
	type ReportsWindow,
	decodeSearch as decodeReports,
	encodeSearch as encodeReports,
} from "../reports/search";

const DAY = 24 * 60 * 60 * 1_000;
// The API reads an alert's history back at most this far.
const MAX_HISTORY_DAYS = 366;
/** All time draws three months of history, the widest preset. */
const ALL_TIME_HISTORY_DAYS = 92;

export type AlertsStatus = "active" | "dismissed" | "all";

const STATUSES: readonly AlertsStatus[] = ["active", "dismissed", "all"];

const FILTERS = ["branch", "testbed", "measure"] as const;
export type Filter = (typeof FILTERS)[number];

/** The list endpoint's name for each filter. */
const LISTS: Readonly<Record<Filter, "branches" | "testbeds" | "measures">> = {
	branch: "branches",
	testbed: "testbeds",
	measure: "measures",
};

/** What the Alerts list shows, as its link carries it; filters are UUIDs, as the API takes them. */
export interface AlertsSearch {
	status: AlertsStatus;
	branch?: string;
	testbed?: string;
	measure?: string;
	window: ReportsWindow;
}

export const DEFAULT_SEARCH: AlertsSearch = {
	status: "active",
	window: { kind: "rolling", days: 28 },
};

// A UUID, hyphenated or simple, as the API parses one.
const UUID =
	/^(?:[0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;

/** Read a link; anything hand edited that does not parse falls back to the default. */
export const decodeSearch = (params: URLSearchParams): AlertsSearch => {
	const search: AlertsSearch = {
		status:
			STATUSES.find((status) => status === params.get("status")) ??
			DEFAULT_SEARCH.status,
		window: decodeReports(params).window,
	};
	for (const filter of FILTERS) {
		const value = params.get(filter);
		if (value && UUID.test(value)) {
			search[filter] = value;
		}
	}
	return search;
};

/** The link's query, defaults left out and keys in a fixed order; the window as the Reports list writes it. */
export const encodeSearch = (search: AlertsSearch) => {
	const params = new URLSearchParams();
	if (search.status !== DEFAULT_SEARCH.status) {
		params.set("status", search.status);
	}
	for (const filter of FILTERS) {
		const value = search[filter];
		if (value) {
			params.set(filter, value);
		}
	}
	const window = encodeReports({ window: search.window, alerts: false });
	return [params.toString(), window].filter(Boolean).join("&");
};

/** The search with one filter set, or cleared when `value` is undefined. */
export const withFilter = (
	search: AlertsSearch,
	filter: Filter,
	value: string | undefined,
): AlertsSearch => {
	const next = { ...search };
	if (value === undefined) {
		delete next[filter];
	} else {
		next[filter] = value;
	}
	return next;
};

/** How many filters narrow the list; the status and the window are not filters. */
export const filterCount = (search: AlertsSearch) =>
	FILTERS.filter((filter) => search[filter] !== undefined).length;

/** Where a batch starts in the list, and how many it holds. */
export interface Batch {
	offset: number;
	perPage: number;
}

/** When the alerts' reports came in, in milliseconds, as one batch bound it. */
export interface Bounds {
	start_time?: number;
	end_time?: number;
}

/** A window's bounds, a rolling one measured back from `now`. */
export const windowBounds = (window: ReportsWindow, now: number): Bounds => {
	switch (window.kind) {
		case "all":
			return {};
		case "rolling":
			return { start_time: now - window.days * DAY };
		case "custom":
			return { start_time: window.start, end_time: window.end };
	}
};

/** How many days of history each alert's row draws, before its report. */
export const historyDays = (window: ReportsWindow) => {
	const days =
		window.kind === "rolling"
			? window.days
			: window.kind === "custom"
				? Math.ceil((window.end - window.start + 1) / DAY)
				: ALL_TIME_HISTORY_DAYS;
	return Math.min(MAX_HISTORY_DAYS, Math.max(1, days));
};

/** The list endpoint's query for one batch. */
export const alertsParams = (
	search: AlertsSearch,
	bounds: Bounds,
	batch: Batch,
) => {
	const params = new URLSearchParams({ status: search.status });
	for (const filter of FILTERS) {
		const value = search[filter];
		if (value) {
			params.set(LISTS[filter], value);
		}
	}
	if (bounds.start_time !== undefined) {
		params.set("start_time", String(bounds.start_time));
	}
	if (bounds.end_time !== undefined) {
		params.set("end_time", String(bounds.end_time));
	}
	params.set("window", String(historyDays(search.window)));
	params.set("offset", String(batch.offset));
	params.set("per_page", String(batch.perPage));
	return params;
};

/**
 * The active alerts the list counts under these filters and bounds, whatever
 * status it shows, up to `read`, when the API read the list.
 */
export const dismissAllFilter = (
	search: AlertsSearch,
	bounds: Bounds,
	read: number,
): JsonAlertsFilter => {
	const filter: JsonAlertsFilter = {
		status: "active" as NonNullable<JsonAlertsFilter["status"]>,
	};
	for (const name of FILTERS) {
		const value = search[name];
		if (value) {
			filter[LISTS[name]] = [value];
		}
	}
	return {
		...filter,
		...bounds,
		end_time: Math.min(bounds.end_time ?? read, read),
	};
};
