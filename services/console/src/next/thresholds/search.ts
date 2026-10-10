import { customWindow, dateValue } from "../reports/search";
import { dayText } from "./model";

const DAY = 24 * 60 * 60 * 1_000;
// The API reads an alert's history back at most this far.
const MAX_HISTORY_DAYS = 366;

/** The window alerts are counted in: the last days, or a range. */
export type AlertsWindow =
	| { kind: "rolling"; days: number }
	| { kind: "custom"; start: number; end: number };

/** The window control's presets, before Custom. */
const PRESETS = [
	{ label: "1w", long: "1 week", days: 7 },
	{ label: "4w", long: "4 weeks", days: 28 },
	{ label: "3m", long: "3 months", days: 92 },
] as const;

const DEFAULT_DAYS = 28;
const DEFAULT_WINDOW: AlertsWindow = { kind: "rolling", days: DEFAULT_DAYS };

/** What the Thresholds list shows, as its link carries it; filters are UUIDs, as the API takes them. */
export interface ThresholdsSearch {
	archived: boolean;
	branch?: string;
	testbed?: string;
	measure?: string;
	window: AlertsWindow;
}

export const DEFAULT_SEARCH: ThresholdsSearch = {
	archived: false,
	window: DEFAULT_WINDOW,
};

type AlertStatus = "active" | "dismissed" | "all";

/** What a threshold's page shows of its alerts, as its link carries it. */
export interface ThresholdView {
	status: AlertStatus;
	window: AlertsWindow;
}

export const DEFAULT_VIEW: ThresholdView = {
	status: "all",
	window: DEFAULT_WINDOW,
};

// A UUID, hyphenated or simple, as the API parses one.
const UUID =
	/^(?:[0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$/i;

export const isUuid = (value: string) => UUID.test(value);

const FILTERS = ["branch", "testbed", "measure"] as const;
export type Filter = (typeof FILTERS)[number];

/** Read a link; anything hand edited that does not parse falls back to the default. */
export const decodeSearch = (params: URLSearchParams): ThresholdsSearch => {
	const search: ThresholdsSearch = {
		archived: params.get("status") === "archived",
		window: decodeWindow(params),
	};
	for (const filter of FILTERS) {
		const value = params.get(filter);
		if (value && isUuid(value)) {
			search[filter] = value;
		}
	}
	return search;
};

/** The link's query, defaults left out and keys in a fixed order. */
export const encodeSearch = (search: ThresholdsSearch) => {
	const params = new URLSearchParams();
	if (search.archived) {
		params.set("status", "archived");
	}
	for (const filter of FILTERS) {
		const value = search[filter];
		if (value) {
			params.set(filter, value);
		}
	}
	encodeWindow(params, search.window);
	return params.toString();
};

const time = (value: string | null) =>
	value && /^\d{1,15}$/.test(value) ? Number(value) : undefined;

const decodeWindow = (params: URLSearchParams): AlertsWindow => {
	const start = time(params.get("start_time"));
	const end = time(params.get("end_time"));
	if (start !== undefined && end !== undefined && start <= end) {
		return { kind: "custom", start, end };
	}
	const preset = PRESETS.find(({ label }) => label === params.get("window"));
	return preset ? { kind: "rolling", days: preset.days } : DEFAULT_WINDOW;
};

const encodeWindow = (params: URLSearchParams, window: AlertsWindow) => {
	if (window.kind === "custom") {
		params.set("start_time", String(window.start));
		params.set("end_time", String(window.end));
		return;
	}
	const preset = PRESETS.find(({ days }) => days === window.days);
	if (preset && window.days !== DEFAULT_DAYS) {
		params.set("window", preset.label);
	}
};

/** The search with one filter set, or cleared when `value` is undefined. */
export const withFilter = (
	search: ThresholdsSearch,
	filter: Filter,
	value: string | undefined,
): ThresholdsSearch => {
	const next = { ...search };
	if (value === undefined) {
		delete next[filter];
	} else {
		next[filter] = value;
	}
	return next;
};

/** How many filters narrow the list; the status and the window are not filters. */
export const filterCount = (search: ThresholdsSearch) =>
	FILTERS.filter((filter) => search[filter] !== undefined).length;

export interface Batch {
	page: number;
	perPage: number;
}

/** The list endpoint's query for one batch, a rolling window measured from `now`. */
export const thresholdsParams = (
	search: ThresholdsSearch,
	now: number,
	batch: Batch,
) => {
	const params = new URLSearchParams();
	if (search.archived) {
		params.set("archived", "true");
	}
	for (const filter of FILTERS) {
		const value = search[filter];
		if (value) {
			params.set(filter, value);
		}
	}
	setWindow(params, search.window, now);
	params.set("page", String(batch.page));
	params.set("per_page", String(batch.perPage));
	return params;
};

const setWindow = (
	params: URLSearchParams,
	window: AlertsWindow,
	now: number,
) => {
	if (window.kind === "rolling") {
		params.set("start_time", String(now - window.days * DAY));
	} else {
		params.set("start_time", String(window.start));
		params.set("end_time", String(window.end));
	}
};

const STATUSES: readonly AlertStatus[] = ["active", "dismissed", "all"];

export const decodeView = (params: URLSearchParams): ThresholdView => ({
	status:
		STATUSES.find((status) => status === params.get("status")) ??
		DEFAULT_VIEW.status,
	window: decodeWindow(params),
});

export const encodeView = (view: ThresholdView) => {
	const params = new URLSearchParams();
	if (view.status !== DEFAULT_VIEW.status) {
		params.set("status", view.status);
	}
	encodeWindow(params, view.window);
	return params.toString();
};

/** How many days of history each alert's row draws, before its report. */
export const historyDays = (window: AlertsWindow) => {
	const days =
		window.kind === "rolling"
			? window.days
			: Math.ceil((window.end - window.start + 1) / DAY);
	return Math.min(MAX_HISTORY_DAYS, Math.max(1, days));
};

/** The alerts endpoint's query for one batch of a threshold's alerts. */
export const alertsParams = (
	threshold: string,
	view: ThresholdView,
	now: number,
	batch: Batch,
) => {
	const params = new URLSearchParams({
		thresholds: threshold,
		status: view.status,
	});
	setWindow(params, view.window, now);
	params.set("window", String(historyDays(view.window)));
	params.set("page", String(batch.page));
	params.set("per_page", String(batch.perPage));
	return params;
};

/** The window inside a sentence: "in the last 4 weeks", "from Aug 18 to Sep 13". */
export const windowPhrase = (
	window: AlertsWindow,
	now: number,
	timeZone?: string,
) => {
	if (window.kind === "custom") {
		return `from ${dayText(window.start, now, timeZone)} to ${dayText(window.end, now, timeZone)}`;
	}
	const { days } = window;
	const preset = PRESETS.find((each) => each.days === days);
	return `in the last ${days === 7 ? "week" : (preset?.long ?? `${days} days`)}`;
};

export type Choice = (typeof PRESETS)[number]["label"] | "custom";

/** The window control's positions. */
export const WINDOW_CHOICES: { value: Choice; label: string; long: string }[] =
	[
		...PRESETS.map(({ label, long }) => ({ value: label, label, long })),
		{ value: "custom", label: "Custom", long: "Custom range" },
	];

/** The window's position on the control. */
export const windowLabel = (window: AlertsWindow): Choice =>
	window.kind === "rolling"
		? (PRESETS.find(({ days }) => days === window.days)?.label ?? "custom")
		: "custom";

/** The window a choice picks: a preset, or the custom range in place, else the last four weeks to now. */
export const pickWindow = (
	choice: Choice,
	current: AlertsWindow,
	now: number,
): AlertsWindow | undefined => {
	const preset = PRESETS.find(({ label }) => label === choice);
	if (preset) {
		return { kind: "rolling", days: preset.days };
	}
	if (current.kind === "custom") {
		return current;
	}
	const range = customWindow(dateValue(now - 27 * DAY), dateValue(now));
	return range?.kind === "custom" ? range : undefined;
};
