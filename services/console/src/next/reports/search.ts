import type { Adapter } from "../../types/bencher";

const DAY = 24 * 60 * 60 * 1_000;
/** Ten years: a longer window is a typo, not a question. */
const MAX_DAYS = 3_650;
const MAX_NAME = 256;

/** An adapter by name, without loading the generated enums. */
export type AdapterName = `${Adapter}`;

export type ReportsWindow =
	| { kind: "rolling"; days: number }
	| { kind: "custom"; start: number; end: number }
	| { kind: "all" };

/** What the Reports list shows, as its link carries it. */
export interface ReportsSearch {
	window: ReportsWindow;
	branch?: string;
	testbed?: string;
	adapter?: AdapterName;
	/** Only reports with an active alert. */
	alerts: boolean;
}

export const DEFAULT_SEARCH: ReportsSearch = {
	window: { kind: "rolling", days: 28 },
	alerts: false,
};

/** The window control's presets, before Custom and All. */
export const WINDOWS = [
	{ label: "1w", long: "1 week", days: 7 },
	{ label: "4w", long: "4 weeks", days: 28 },
	{ label: "3m", long: "3 months", days: 92 },
] as const;

/**
 * Every adapter a report is listed with, by name. A language adapter such as
 * `rust` tries each of its tools in turn, so the API lists it as `magic`.
 */
export const ADAPTERS: readonly AdapterName[] = [
	"c_sharp_dot_net",
	"cpp_catch2",
	"cpp_google",
	"dart_benchmark_harness",
	"go_bench",
	"java_jmh",
	"js_benchmark",
	"js_time",
	"js_vitest",
	"json",
	"json_v0",
	"json_v1",
	"magic",
	"python_asv",
	"python_pytest",
	"ruby_benchmark",
	"rust_bench",
	"rust_criterion",
	"rust_gungraun",
	"rust_gungraun_json",
	"rust_gungraun_stdout",
	"rust_iai",
	"shell_hyperfine",
];

const LISTED = new Set<string>(ADAPTERS);

const name = (value: string | null) => {
	const trimmed = value?.trim();
	return trimmed && trimmed.length <= MAX_NAME ? trimmed : undefined;
};

const time = (value: string | null) =>
	value && /^\d{1,15}$/.test(value) ? Number(value) : undefined;

const decodeWindow = (params: URLSearchParams): ReportsWindow => {
	const start = time(params.get("start_time"));
	const end = time(params.get("end_time"));
	if (start !== undefined && end !== undefined && start <= end) {
		return { kind: "custom", start, end };
	}
	const window = params.get("window");
	if (window === "all") {
		return { kind: "all" };
	}
	const rolling = window?.match(/^(\d{1,5})([dw])$/);
	if (rolling) {
		const days = Number(rolling[1]) * (rolling[2] === "w" ? 7 : 1);
		if (days > 0 && days <= MAX_DAYS) {
			return { kind: "rolling", days };
		}
	}
	return DEFAULT_SEARCH.window;
};

/** Read a link; anything hand edited that does not parse falls back to the default. */
export const decodeSearch = (params: URLSearchParams): ReportsSearch => {
	const search: ReportsSearch = {
		window: decodeWindow(params),
		alerts: params.get("alerts") === "active",
	};
	const branch = name(params.get("branch"));
	if (branch) {
		search.branch = branch;
	}
	const testbed = name(params.get("testbed"));
	if (testbed) {
		search.testbed = testbed;
	}
	const adapter = params.get("adapter");
	if (adapter && LISTED.has(adapter)) {
		search.adapter = adapter as AdapterName;
	}
	return search;
};

const isDefaultWindow = (window: ReportsWindow) =>
	window.kind === "rolling" &&
	DEFAULT_SEARCH.window.kind === "rolling" &&
	window.days === DEFAULT_SEARCH.window.days;

/** The link's query, defaults left out and keys in a fixed order. */
export const encodeSearch = (search: ReportsSearch) => {
	const params = new URLSearchParams();
	if (search.branch) {
		params.set("branch", search.branch);
	}
	if (search.testbed) {
		params.set("testbed", search.testbed);
	}
	if (search.adapter) {
		params.set("adapter", search.adapter);
	}
	if (search.alerts) {
		params.set("alerts", "active");
	}
	const { window } = search;
	if (window.kind === "all") {
		params.set("window", "all");
	} else if (window.kind === "custom") {
		params.set("start_time", String(window.start));
		params.set("end_time", String(window.end));
	} else if (!isDefaultWindow(window)) {
		params.set(
			"window",
			window.days % 7 === 0 ? `${window.days / 7}w` : `${window.days}d`,
		);
	}
	return params.toString();
};

type Filter = "branch" | "testbed" | "adapter";

/** The search with one filter set, or cleared when `value` is undefined. */
export const withFilter = <K extends Filter>(
	search: ReportsSearch,
	key: K,
	value: ReportsSearch[K] | undefined,
): ReportsSearch => {
	const next = { ...search };
	if (value === undefined) {
		delete next[key];
	} else {
		next[key] = value as ReportsSearch[K];
	}
	return next;
};

export interface Batch {
	page: number;
	perPage: number;
}

/** The list endpoint's query for one batch, a rolling window measured from `now`. */
export const apiParams = (search: ReportsSearch, now: number, batch: Batch) => {
	const params = new URLSearchParams();
	if (search.branch) {
		params.set("branch", search.branch);
	}
	if (search.testbed) {
		params.set("testbed", search.testbed);
	}
	if (search.adapter) {
		params.set("adapter", search.adapter);
	}
	if (search.alerts) {
		params.set("active_alerts", "true");
	}
	const { window } = search;
	if (window.kind === "rolling") {
		params.set("start_time", String(now - window.days * DAY));
	} else if (window.kind === "custom") {
		params.set("start_time", String(window.start));
		params.set("end_time", String(window.end));
	}
	params.set("page", String(batch.page));
	params.set("per_page", String(batch.perPage));
	return params;
};

/** How many filters narrow the list; the window is not one. */
export const filterCount = (search: ReportsSearch) =>
	[search.branch, search.testbed, search.adapter, search.alerts].filter(Boolean)
		.length;

const span = (days: number) => {
	const preset = WINDOWS.find((window) => window.days === days);
	if (preset) {
		return preset.days === 7 ? "week" : preset.long;
	}
	if (days === 1) {
		return "day";
	}
	return days % 7 === 0 ? `${days / 7} weeks` : `${days} days`;
};

const dayFormat = (withYear: boolean, timeZone?: string) =>
	new Intl.DateTimeFormat("en-US", {
		month: "short",
		day: "numeric",
		...(withYear ? { year: "numeric" } : {}),
		timeZone,
	});

const yearOf = (time: number, timeZone?: string) =>
	new Intl.DateTimeFormat("en-US", { year: "numeric", timeZone }).format(time);

/** The window as the page head names it. */
export const windowPhrase = (
	window: ReportsWindow,
	now: number,
	timeZone?: string,
) => {
	switch (window.kind) {
		case "all":
			return "All time";
		case "rolling":
			return `In the last ${span(window.days)}`;
		case "custom": {
			const thisYear = yearOf(now, timeZone);
			const withYear =
				yearOf(window.start, timeZone) !== thisYear ||
				yearOf(window.end, timeZone) !== thisYear;
			const format = dayFormat(withYear, timeZone);
			return `From ${format.format(window.start)} to ${format.format(window.end)}`;
		}
	}
};

const and = (parts: string[]) =>
	parts.length > 1
		? `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)}`
		: (parts[0] ?? "");

/** The names of the branch and the testbed the search filters by, where known. */
export interface FilterNames {
	branch?: string | undefined;
	testbed?: string | undefined;
}

/** What the zero state says: which filters and window found nothing, and what to try. */
export const nothingFound = (
	search: ReportsSearch,
	now: number,
	timeZone?: string,
	names: FilterNames = {},
) => {
	const where = [
		search.branch && (names.branch ?? search.branch),
		search.testbed && (names.testbed ?? search.testbed),
	].filter((part): part is string => Boolean(part));
	const what = [
		...(search.adapter ? [`the ${search.adapter} adapter`] : []),
		...(search.alerts ? ["active alerts"] : []),
	];
	const phrase = [
		"Nothing",
		...(where.length ? [`on ${and(where)}`] : []),
		...(what.length ? [`with ${and(what)}`] : []),
		...(search.window.kind === "all"
			? []
			: [
					windowPhrase(search.window, now, timeZone).replace(
						/^(In|From)/,
						(word) => word.toLowerCase(),
					),
				]),
	].join(" ");
	const filtered = filterCount(search) > 0;
	const advice =
		search.window.kind === "all"
			? "Clear the filters."
			: filtered
				? "Widen the window or clear the filters."
				: "Widen the window.";
	return `${phrase}. ${advice}`;
};

const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

/** Whole local days from `from` through `to`, as a date input gives them. */
export const customWindow = (
	from: string,
	to: string,
): ReportsWindow | undefined => {
	const start = DATE.exec(from);
	const end = DATE.exec(to);
	if (!(start && end)) {
		return undefined;
	}
	const day = ([, y, m, d]: RegExpExecArray, offset = 0) =>
		new Date(Number(y), Number(m) - 1, Number(d) + offset).getTime();
	const window = {
		kind: "custom",
		start: day(start),
		end: day(end, 1) - 1,
	} as const;
	return window.start <= window.end ? window : undefined;
};

/** A time as a date input's local value. */
export const dateValue = (time: number) => {
	const date = new Date(time);
	const pad = (n: number) => String(n).padStart(2, "0");
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
};
