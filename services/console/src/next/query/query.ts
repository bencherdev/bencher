import { PerfQueryKey, PlotKey } from "../../types/bencher";

export type ParameterValue = string | number | boolean;
export type Parameters = Readonly<Record<string, ParameterValue>>;

/** A branch in the query, and the head to draw when not its current one. */
export interface BranchEntry {
	readonly uuid: string;
	readonly head?: string;
}

/** A testbed in the query, and the spec to draw when not every one. */
export interface TestbedEntry {
	readonly uuid: string;
	readonly spec?: string;
}

type XAxis = "date" | "version";
type YScale = "auto" | "linear" | "log";
export type Layout = "dual" | "stacked";

/** A rolling window of `seconds` that ends at `end` or now, or a custom range. */
export type QueryWindow =
	| { readonly seconds: number; readonly end?: number }
	| { readonly start: number; readonly end?: number };

/** Explore's query: the six boxes, the view, and the key's hidden and focused lines. */
export interface ExploreQuery {
	readonly branches: readonly BranchEntry[];
	readonly testbeds: readonly TestbedEntry[];
	/** Benchmark UUIDs. */
	readonly benchmarks: readonly string[];
	/** The parameters box: a variant is drawn when it carries every tag of any set. */
	readonly sets: readonly Parameters[];
	/** Measure UUIDs. */
	readonly measures: readonly string[];
	/** Metric names; none draws every metric name in the window. */
	readonly metrics: readonly string[];
	readonly xAxis: XAxis;
	readonly yScale: YScale;
	readonly window: QueryWindow;
	/** The reader's choice; `measuresLayout` decides what is drawn. */
	readonly layout: Layout;
	/** Line keys hidden in the key. */
	readonly hide: readonly string[];
	/** Line keys that alone are drawn: a selection names its lines without knowing the extras. */
	readonly only?: readonly string[];
	readonly focus?: string;
	/** The report the query was opened from. */
	readonly report?: string;
	/** The pinned plot the query edits. */
	readonly plot?: string;
}

const DAY = 24 * 60 * 60;
const WEEK = 7 * DAY;
const DEFAULT_WINDOW = 4 * WEEK;
const MAX_WINDOW = 2 ** 32 - 1;

/** The perf query reads at most eight values of each box and eight sets. */
export const MAX_ENTRIES = 8;
const MAX_PARAMETER_KEYS = 8;
const MAX_NAME_BYTES = 64;

// The classic perf page's names, so a classic link reads by path alone.
const BRANCHES = PerfQueryKey.Branches;
const HEADS = PerfQueryKey.Heads;
const TESTBEDS = PerfQueryKey.Testbeds;
const SPECS = PerfQueryKey.Specs;
const BENCHMARKS = PerfQueryKey.Benchmarks;
const MEASURES = PerfQueryKey.Measures;
const START_TIME = PerfQueryKey.StartTime;
const END_TIME = PerfQueryKey.EndTime;
const X_AXIS = PlotKey.XAxis;
const Y_AXIS = PlotKey.YAxis;
const RANGE = "range";
const REPORT = "report";
const PLOT = "plot";
// The perf query's names, one set or metric name to each.
const PARAMETERS = PerfQueryKey.Parameters;
const METRICS = "metrics";
// Explore's own.
const WINDOW = "window";
const LAYOUT = "layout";
const HIDE = "hide";
const ONLY = "only";
const FOCUS = "focus";

const X_AXES = new Map<string | null, XAxis>([
	["date_time", "date"],
	["version", "version"],
]);
const Y_SCALES: readonly YScale[] = ["auto", "linear", "log"];

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const LINE_KEY = /^[0-9a-z]{1,11}$/;
// Fifteen digits stay below 2^53, so every time reads exactly.
const TIME = /^\d{1,15}$/;
const ROLLING = /^(\d{1,10})([wds])$/;
const UNIT_SECONDS = new Map([
	["w", WEEK],
	["d", DAY],
	["s", 1],
]);

export const blankQuery = (): ExploreQuery => ({
	branches: [],
	testbeds: [],
	benchmarks: [],
	sets: [],
	measures: [],
	metrics: [],
	xAxis: "date",
	yScale: "auto",
	window: { seconds: DEFAULT_WINDOW },
	layout: "dual",
	hide: [],
});

/** The set's JSON with its keys in RFC 8785 order, as the API canonicalizes it. */
export const canonicalParameters = (parameters: Parameters): string =>
	`{${Object.keys(parameters)
		// The default sort compares UTF-16 code units, which is RFC 8785's order.
		.sort()
		.map((key) => `${JSON.stringify(key)}:${JSON.stringify(parameters[key])}`)
		.join(",")}}`;

/** The first of each identity, at most `max` of them. */
export const distinct = <T>(
	items: readonly T[],
	identity: (item: T) => string,
	max = Number.POSITIVE_INFINITY,
): T[] => {
	const seen = new Set<string>();
	const kept: T[] = [];
	for (const item of items) {
		const id = identity(item);
		if (kept.length < max && !seen.has(id)) {
			seen.add(id);
			kept.push(item);
		}
	}
	return kept;
};

export const itself = (text: string): string => text;

export const encodeQuery = (query: ExploreQuery): string => {
	const pairs: [string, string][] = [];
	const add = (name: string, value: string) => pairs.push([name, value]);
	const addList = (name: string, values: readonly string[]) => {
		if (values.length > 0) {
			add(name, values.join(","));
		}
	};
	const { branches, testbeds, window } = query;
	addList(
		BRANCHES,
		branches.map(({ uuid }) => uuid),
	);
	if (branches.some(({ head }) => head !== undefined)) {
		add(HEADS, branches.map(({ head }) => head ?? "").join(","));
	}
	addList(
		TESTBEDS,
		testbeds.map(({ uuid }) => uuid),
	);
	if (testbeds.some(({ spec }) => spec !== undefined)) {
		add(SPECS, testbeds.map(({ spec }) => spec ?? "").join(","));
	}
	addList(BENCHMARKS, query.benchmarks);
	for (const set of query.sets) {
		add(PARAMETERS, canonicalParameters(set));
	}
	addList(MEASURES, query.measures);
	for (const metric of query.metrics) {
		add(METRICS, metric);
	}
	if ("start" in window) {
		add(START_TIME, String(window.start));
	} else if (window.seconds !== DEFAULT_WINDOW) {
		add(WINDOW, rollingText(window.seconds));
	}
	if (window.end !== undefined) {
		add(END_TIME, String(window.end));
	}
	if (query.xAxis === "version") {
		add(X_AXIS, "version");
	}
	if (query.yScale !== "auto") {
		add(Y_AXIS, query.yScale);
	}
	if (query.layout === "stacked") {
		add(LAYOUT, "stacked");
	}
	addList(HIDE, query.hide);
	addList(ONLY, query.only ?? []);
	if (query.focus !== undefined) {
		add(FOCUS, query.focus);
	}
	if (query.report !== undefined) {
		add(REPORT, query.report);
	}
	if (query.plot !== undefined) {
		add(PLOT, query.plot);
	}
	if (pairs.length === 0) {
		return "";
	}
	return `?${pairs.map(([name, value]) => `${name}=${component(value)}`).join("&")}`;
};

// Commas, colons, and braces read fine in a query, so a link keeps them legible.
const component = (value: string): string =>
	encodeURIComponent(value).replace(/%(2C|3A|7B|7D)/g, (triplet) =>
		decodeURIComponent(triplet),
	);

const rollingText = (seconds: number): string => {
	if (seconds % WEEK === 0) {
		return `${seconds / WEEK}w`;
	}
	if (seconds % DAY === 0) {
		return `${seconds / DAY}d`;
	}
	return `${seconds}s`;
};

export const decodeQuery = (search: string): ExploreQuery => {
	const params = new URLSearchParams(search);
	// Decode, then split: a link rewritten along the way may carry its commas encoded.
	const list = (name: string): string[] => params.get(name)?.split(",") ?? [];
	const uuids = (name: string) =>
		distinct(list(name).filter(isUuid), itself, MAX_ENTRIES);
	const keys = (name: string) => distinct(list(name).filter(isLineKey), itself);
	const branches = paired(list(BRANCHES), list(HEADS)).map(
		([uuid, head]): BranchEntry =>
			head === undefined ? { uuid } : { uuid, head },
	);
	const testbeds = paired(list(TESTBEDS), list(SPECS)).map(
		([uuid, spec]): TestbedEntry =>
			spec === undefined ? { uuid } : { uuid, spec },
	);
	const only = keys(ONLY);
	const focus = params.get(FOCUS);
	const report = params.get(REPORT);
	const plot = params.get(PLOT);
	return {
		branches,
		testbeds,
		benchmarks: uuids(BENCHMARKS),
		sets: params
			.getAll(PARAMETERS)
			.flatMap(readParameters)
			.slice(0, MAX_ENTRIES),
		measures: uuids(MEASURES),
		metrics: distinct(
			params.getAll(METRICS).filter(isName),
			itself,
			MAX_ENTRIES,
		),
		xAxis:
			X_AXES.get(params.get(X_AXIS)) ?? X_AXES.get(params.get(RANGE)) ?? "date",
		yScale: Y_SCALES.find((scale) => scale === params.get(Y_AXIS)) ?? "auto",
		window: readWindow(params),
		layout: params.get(LAYOUT) === "stacked" ? "stacked" : "dual",
		hide: keys(HIDE),
		...(only.length > 0 ? { only } : {}),
		...(focus !== null && isLineKey(focus) ? { focus } : {}),
		...(report !== null && isUuid(report) ? { report } : {}),
		...(plot !== null && isUuid(plot) ? { plot } : {}),
	};
};

/** Each UUID with the entry at its position in the list beside it, paired before either is dropped. */
const paired = (
	uuids: readonly string[],
	beside: readonly string[],
): [string, string | undefined][] =>
	distinct(
		uuids.flatMap((uuid, index): [string, string | undefined][] => {
			const other = beside[index];
			return isUuid(uuid)
				? [[uuid, other !== undefined && isUuid(other) ? other : undefined]]
				: [];
		}),
		([uuid]) => uuid,
		MAX_ENTRIES,
	);

const readWindow = (params: URLSearchParams): QueryWindow => {
	const start = readTime(params.get(START_TIME));
	const end = readTime(params.get(END_TIME));
	const ending = end === undefined ? {} : { end };
	if (start !== undefined) {
		return { start, ...ending };
	}
	return {
		seconds: readRolling(params.get(WINDOW)) ?? DEFAULT_WINDOW,
		...ending,
	};
};

const readTime = (text: string | null): number | undefined =>
	text !== null && TIME.test(text) ? Number(text) : undefined;

const readRolling = (text: string | null): number | undefined => {
	const [, count, unit] = ROLLING.exec(text ?? "") ?? [];
	const seconds = Number(count) * (UNIT_SECONDS.get(unit ?? "") ?? 0);
	return seconds >= 1 && seconds <= MAX_WINDOW ? seconds : undefined;
};

const readParameters = (text: string): Parameters[] => {
	let value: unknown;
	try {
		value = JSON.parse(text);
	} catch {
		return [];
	}
	if (typeof value !== "object" || value === null || Array.isArray(value)) {
		return [];
	}
	const entries = Object.entries(value);
	const tags = entries.filter(
		(entry): entry is [string, ParameterValue] =>
			isName(entry[0]) && isScalar(entry[1]),
	);
	return tags.length === entries.length && tags.length <= MAX_PARAMETER_KEYS
		? [Object.fromEntries(tags)]
		: [];
};

const isScalar = (value: unknown): value is ParameterValue =>
	typeof value === "boolean" ||
	(typeof value === "number" && Number.isFinite(value)) ||
	(typeof value === "string" && isName(value));

const encoder = new TextEncoder();

/** A name as the API takes one: non-empty, trimmed, and at most 64 bytes. */
const isName = (text: string): boolean =>
	text.length > 0 &&
	text === text.trim() &&
	encoder.encode(text).length <= MAX_NAME_BYTES;

const isUuid = (text: string): boolean => UUID.test(text);

const isLineKey = (text: string): boolean => LINE_KEY.test(text);
