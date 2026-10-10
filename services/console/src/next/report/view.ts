type ReportGroup = "benchmark" | "measure";
type ReportSort = "name" | "delta";
/** A preset, or a custom number of days. */
type ReportWindow = WindowPreset | number;
type WindowPreset = "1w" | "4w" | "3m";

/** What a report page's URL holds; the selection stays out of it. */
export interface ReportView {
	group: ReportGroup;
	sort: ReportSort;
	window: ReportWindow;
	search: string;
	expanded: string[];
}

// The API reads a report's history back at most this far.
const MAX_WINDOW_DAYS = 366;

const PRESET_DAYS: Readonly<Record<WindowPreset, number>> = {
	"1w": 7,
	"4w": 28,
	"3m": 92,
};

const DEFAULT_WINDOW = "4w";

const CUSTOM_WINDOW = /^([1-9][0-9]*)d$/;

export const parseReportView = (params: URLSearchParams): ReportView => ({
	group: params.get("group") === "measure" ? "measure" : "benchmark",
	sort: params.get("sort") === "delta" ? "delta" : "name",
	window: parseWindow(params.get("window")),
	search: params.get("search")?.trim() ?? "",
	expanded: [...new Set(params.getAll("expanded"))],
});

/** The search string for a view, empty for the default view and without a leading `?`. */
export const reportViewSearch = (view: ReportView): string => {
	const params = new URLSearchParams();
	if (view.group !== "benchmark") {
		params.set("group", view.group);
	}
	if (view.sort !== "name") {
		params.set("sort", view.sort);
	}
	if (view.window !== DEFAULT_WINDOW) {
		params.set(
			"window",
			typeof view.window === "number" ? `${view.window}d` : view.window,
		);
	}
	const search = view.search.trim();
	if (search) {
		params.set("search", search);
	}
	for (const key of view.expanded) {
		params.append("expanded", key);
	}
	return params.toString();
};

const parseWindow = (window: string | null): ReportWindow => {
	if (window !== null && Object.hasOwn(PRESET_DAYS, window)) {
		return window as WindowPreset;
	}
	const days = CUSTOM_WINDOW.exec(window ?? "")?.[1];
	return days === undefined ? DEFAULT_WINDOW : Number(days);
};

/** How many days before the report a window reaches. */
export const windowDays = (window: ReportWindow): number =>
	typeof window === "number"
		? Math.min(MAX_WINDOW_DAYS, window)
		: PRESET_DAYS[window];
