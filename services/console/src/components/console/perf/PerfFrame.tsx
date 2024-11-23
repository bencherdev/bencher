import * as Sentry from "@sentry/astro";
import { debounce } from "@solid-primitives/scheduled";
import type { Params } from "astro";
import {
	Show,
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { createStore } from "solid-js/store";
import { PerfTab, isPerfTab, isXAxis } from "../../../config/types";
import {
	type JsonBenchmark,
	type JsonBranch,
	type JsonPerf,
	type JsonPerfQuery,
	type JsonPlot,
	type JsonProject,
	type JsonReport,
	type JsonTestbed,
	PerfQueryKey,
	PlotKey,
	XAxis,
} from "../../../types/bencher";
import { authUser } from "../../../util/auth";
import {
	addToArray,
	arrayFromString,
	arrayToString,
	dateTimeMillis,
	dateToTime,
	isBoolParam,
	removeFromArray,
	sizeArray,
	timeToDate,
	timeToDateOnlyIso,
} from "../../../util/convert";
import { X_TOTAL_COUNT, httpGet } from "../../../util/http";
import {
	MAX_NOTIFY_TIMEOUT,
	NOTIFY_TIMEOUT_PARAM,
	NotifyKind,
	pageNotify,
} from "../../../util/notify";
import { useSearchParams } from "../../../util/url";
import { DEBOUNCE_DELAY, validU32 } from "../../../util/valid";
import { themeSignal } from "../../navbar/theme/util";
import PerfHeader from "./header/PerfHeader";
import PerfPlot from "./plot/PerfPlot";
import type { TabList } from "./plot/tab/PlotTab";

// Perf query params
const BRANCHES_PARAM = PerfQueryKey.Branches;
const HEADS_PARAM = PerfQueryKey.Heads;
const TESTBEDS_PARAM = PerfQueryKey.Testbeds;
const BENCHMARKS_PARAM = PerfQueryKey.Benchmarks;
const MEASURES_PARAM = PerfQueryKey.Measures;
const START_TIME_PARAM = PerfQueryKey.StartTime;
const END_TIME_PARAM = PerfQueryKey.EndTime;

// Console UI state query params
const REPORT_PARAM = "report";
const PLOT_PARAM = "plot";

const REPORTS_PER_PAGE_PARAM = "reports_per_page";
const BRANCHES_PER_PAGE_PARAM = "branches_per_page";
const TESTBEDS_PER_PAGE_PARAM = "testbeds_per_page";
const BENCHMARKS_PER_PAGE_PARAM = "benchmarks_per_page";
const PLOTS_PER_PAGE_PARAM = "plots_per_page";

const REPORTS_PAGE_PARAM = "reports_page";
const BRANCHES_PAGE_PARAM = "branches_page";
const TESTBEDS_PAGE_PARAM = "testbeds_page";
const BENCHMARKS_PAGE_PARAM = "benchmarks_page";
const PLOTS_PAGE_PARAM = "plots_page";

const REPORTS_START_TIME_PARAM = "reports_start_time";
const REPORTS_END_TIME_PARAM = "reports_end_time";
const BRANCHES_SEARCH_PARAM = "branches_search";
const TESTBEDS_SEARCH_PARAM = "testbeds_search";
const BENCHMARKS_SEARCH_PARAM = "benchmarks_search";
const PLOTS_SEARCH_PARAM = "plots_search";

const TAB_PARAM = "tab";
const KEY_PARAM = "key";
const LOWER_VALUE_PARAM = PlotKey.LowerValue;
const UPPER_VALUE_PARAM = PlotKey.UpperValue;
const LOWER_BOUNDARY_PARAM = PlotKey.LowerBoundary;
const UPPER_BOUNDARY_PARAM = PlotKey.UpperBoundary;
const X_AXIS_PARAM = PlotKey.XAxis;
// TODO remove in due time
const RANGE_PARAM = "range";
const CLEAR_PARAM = "clear";

// These are currently for internal use only
// TODO add a way to set these in the Share modal
// The title can be set but there is no way to set an empty title
const EMBED_LOGO_PARAM = "embed_logo";
export const EMBED_TITLE_PARAM = "embed_title";
const EMBED_HEADER_PARAM = "embed_header";
const EMBED_KEY_PARAM = "embed_key";

// This is used to trim down the number of query params when embedding, etc.
export const PERF_QUERY_PARAMS = [
	BRANCHES_PARAM,
	HEADS_PARAM,
	TESTBEDS_PARAM,
	BENCHMARKS_PARAM,
	MEASURES_PARAM,
	START_TIME_PARAM,
	END_TIME_PARAM,
];
export const PERF_PLOT_PARAMS = [
	...PERF_QUERY_PARAMS,
	REPORT_PARAM,
	REPORTS_PER_PAGE_PARAM,
	BRANCHES_PER_PAGE_PARAM,
	TESTBEDS_PER_PAGE_PARAM,
	BENCHMARKS_PER_PAGE_PARAM,
	PLOTS_PER_PAGE_PARAM,
	REPORTS_PAGE_PARAM,
	BRANCHES_PAGE_PARAM,
	TESTBEDS_PAGE_PARAM,
	BENCHMARKS_PAGE_PARAM,
	PLOTS_PAGE_PARAM,
	REPORTS_START_TIME_PARAM,
	REPORTS_END_TIME_PARAM,
	BRANCHES_SEARCH_PARAM,
	TESTBEDS_SEARCH_PARAM,
	BENCHMARKS_SEARCH_PARAM,
	PLOTS_SEARCH_PARAM,
	TAB_PARAM,
	KEY_PARAM,
	X_AXIS_PARAM,
	CLEAR_PARAM,
	LOWER_VALUE_PARAM,
	UPPER_VALUE_PARAM,
	LOWER_BOUNDARY_PARAM,
	UPPER_BOUNDARY_PARAM,
];
export const PERF_PLOT_EMBED_PARAMS = [
	...PERF_PLOT_PARAMS,
	EMBED_LOGO_PARAM,
	EMBED_TITLE_PARAM,
	EMBED_HEADER_PARAM,
	EMBED_KEY_PARAM,
];
export const PERF_PLOT_PIN_PARAMS = [
	BRANCHES_PARAM,
	TESTBEDS_PARAM,
	BENCHMARKS_PARAM,
	MEASURES_PARAM,
	LOWER_VALUE_PARAM,
	UPPER_VALUE_PARAM,
	LOWER_BOUNDARY_PARAM,
	UPPER_BOUNDARY_PARAM,
];

const DEFAULT_PERF_TAB = PerfTab.REPORTS;
const DEFAULT_PERF_KEY = true;
const DEFAULT_X_AXIS = XAxis.DateTime;
const DEFAULT_PERF_CLEAR = false;
const DEFAULT_PERF_END_VALUE = false;
const DEFAULT_PERF_BOUNDARY = false;

const DEFAULT_EMBED_LOGO = true;
const DEFAULT_EMBED_HEADER = true;
const DEFAULT_EMBED_KEY = true;

export const DEFAULT_PER_PAGE = 8;
export const REPORTS_PER_PAGE = 4;
export const DEFAULT_PAGE = 1;

// 30 days
const DEFAULT_REPORT_HISTORY = 30 * 24 * 60 * 60 * 1000;

export interface Props {
	apiUrl: string;
	params: Params;
	isConsole?: boolean;
	isEmbed?: boolean;
	project?: undefined | JsonProject;
	//
	project_slug: () => string;
	setSearchParams: (params: object, options?: object) => void;
	//
	report: () => JsonReport | null;
	measures: () => string[];
	branches: () => string[];
	heads: () => string[];
	testbeds: () => string[];
	benchmarks: () => string[];
	plot: () => string | null;
	start_time: () => string;
	end_time: () => string;
	start_date: () => string;
	end_date: () => string;
	//
	tab: () => PerfTab;
	key: () => boolean;
	x_axis: () => XAxis;
	clear: () => boolean;
	lower_value: () => boolean;
	upper_value: () => boolean;
	lower_boundary: () => boolean;
	upper_boundary: () => boolean;
	//
	reports_per_page: () => number;
	branches_per_page: () => number;
	testbeds_per_page: () => number;
	benchmarks_per_page: () => number;
	plots_per_page: () => number;
	//
	reports_page: () => number;
	branches_page: () => number;
	testbeds_page: () => number;
	benchmarks_page: () => number;
	plots_page: () => number;
	//
	reports_start_time: () => string;
	reports_end_time: () => string;
	reports_start_date: () => string;
	reports_end_date: () => string;
	branches_search: () => string;
	testbeds_search: () => string;
	benchmarks_search: () => string;
	plots_search: () => string;
	//
	embed_logo: () => boolean;
	embed_title: () => string;
	embed_header: () => boolean;
	embed_key: () => boolean;
}

function resourcesToCheckable<T>(
	resources: { uuid: string }[],
	params: (undefined | string)[],
): TabList<T> {
	return resources.map((resource) => {
		return {
			resource: resource as T,
			checked: params.includes(resource?.uuid),
		};
	});
}

const PerfFrame = (props: Props) => {
	const user = authUser();
	const theme = themeSignal;

	// The perf query sent to the server
	const perfQuery = createMemo(() => {
		return {
			branches: props.branches(),
			heads: props.heads(),
			testbeds: props.testbeds(),
			benchmarks: props.benchmarks(),
			measures: props.measures(),
			start_time: props.start_time(),
			end_time: props.end_time(),
		} as JsonPerfQuery;
	});

	const isPlotInit = createMemo(
		() =>
			props.branches().length === 0 ||
			props.testbeds().length === 0 ||
			props.benchmarks().length === 0 ||
			props.measures().length === 0,
	);

	// Refresh pref query
	const [refresh, setRefresh] = createSignal(0);
	const handleRefresh = () => {
		setRefresh(refresh() + 1);
	};

	const projectFetcher = createMemo(() => {
		return {
			project_slug: props.project_slug(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const getProject = async (fetcher: {
		project_slug: string;
		refresh: number;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		if (props.isConsole && typeof fetcher.token !== "string") {
			return EMPTY_OBJECT;
		}
		if (props.project) {
			return props.project;
		}
		if (!fetcher.project_slug || fetcher.project_slug === "undefined") {
			return EMPTY_OBJECT;
		}
		const path = `/v0/projects/${fetcher.project_slug}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data as JsonProject;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return EMPTY_OBJECT;
			});
	};
	const [project] = createResource<JsonProject>(projectFetcher, getProject);

	const perfFetcher = createMemo(() => {
		return {
			project_slug: props.project_slug(),
			perfQuery: perfQuery(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const getPerf = async (fetcher: {
		project_slug: string;
		perfQuery: JsonPerfQuery;
		refresh: number;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		// Don't even send query if there isn't at least one: branch, testbed, and benchmark
		if (
			(props.isConsole && typeof fetcher.token !== "string") ||
			isPlotInit() ||
			!fetcher.project_slug ||
			fetcher.project_slug === "undefined"
		) {
			return EMPTY_OBJECT;
		}

		const searchParams = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.perfQuery)) {
			if (value) {
				searchParams.set(key, value.toString());
			}
		}
		const path = `/v0/projects/${
			fetcher.project_slug
		}/perf?${searchParams.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				// If the URL is exactly 2000 characters, then it may have been truncated by the browser.
				// There isn't much that we can do other than notify the user.
				if (window.location.href.length === 2000) {
					pageNotify(
						NotifyKind.ERROR,
						"This URL is exactly 2,000 characters. It may have been truncated by your web browser. Please, try opening the original link in a different web browser.",
						{ [NOTIFY_TIMEOUT_PARAM]: MAX_NOTIFY_TIMEOUT },
					);
				} else {
					pageNotify(
						NotifyKind.ERROR,
						"Lettuce romaine calm! Failed to get perf. Please, try again.",
					);
				}
				return EMPTY_OBJECT;
			});
	};
	const [perfData] = createResource<JsonPerf>(perfFetcher, getPerf);

	// Initialize as empty, wait for resources to load
	const [reports_tab, setReportsTab] = createStore<TabList<JsonReport>>([]);
	const [branches_tab, setBranchesTab] = createStore<TabList<JsonBranch>>([]);
	const [testbeds_tab, setTestbedsTab] = createStore<TabList<JsonTestbed>>([]);
	const [benchmarks_tab, setBenchmarksTab] = createStore<
		TabList<JsonBenchmark>
	>([]);
	const [plots_tab, setPlotsTab] = createStore<TabList<JsonPlot>>([]);

	const [reportsTotalCount, setReportsTotalCount] = createSignal(0);
	const [branchesTotalCount, setBranchesTotalCount] = createSignal(0);
	const [testbedsTotalCount, setTestbedsTotalCount] = createSignal(0);
	const [benchmarksTotalCount, setBenchmarksTotalCount] = createSignal(0);
	const [plotsTotalCount, setPlotsTotalCount] = createSignal(0);

	// Resource tabs data: Reports, Branches, Testbeds, Benchmarks, Plots
	async function getPerfTab<T>(
		perfTab: PerfTab,
		fetcher: {
			project_slug: undefined | string;
			per_page: number;
			page: number;
			start_time?: undefined | string;
			end_time?: undefined | string;
			search?: undefined | string;
			refresh: number;
			token: string;
		},
		totalCount: (headers: object) => void,
	) {
		const EMPTY_ARRAY: T[] = [];
		if (!fetcher.project_slug) {
			return EMPTY_ARRAY;
		}
		if (props.isConsole && typeof fetcher.token !== "string") {
			return EMPTY_ARRAY;
		}
		if (props.isEmbed === true) {
			return EMPTY_ARRAY;
		}
		if (!validU32(fetcher.per_page.toString())) {
			return EMPTY_ARRAY;
		}
		if (!validU32(fetcher.page.toString())) {
			return EMPTY_ARRAY;
		}
		const search_params = new URLSearchParams();
		search_params.set("per_page", fetcher.per_page.toString());
		search_params.set("page", fetcher.page.toString());
		if (fetcher.start_time) {
			search_params.set("start_time", fetcher.start_time);
		}
		if (fetcher.end_time) {
			search_params.set("end_time", fetcher.end_time);
		}
		if (fetcher.search) {
			search_params.set("search", fetcher.search.trim());
		}
		const path = `/v0/projects/${
			fetcher.project_slug
		}/${perfTab}?${search_params.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				totalCount(resp?.headers);
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return EMPTY_ARRAY;
			});
	}

	const reports_fetcher = createMemo(() => {
		return {
			project_slug: props.project_slug(),
			per_page: props.reports_per_page(),
			page: props.reports_page(),
			start_time: props.reports_start_time(),
			end_time: props.reports_end_time(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [reports_data] = createResource(reports_fetcher, async (fetcher) =>
		getPerfTab<JsonReport>(PerfTab.REPORTS, fetcher, (headers) =>
			setReportsTotalCount(headers?.[X_TOTAL_COUNT]),
		),
	);
	createEffect(() => {
		const data = reports_data();
		if (data) {
			setReportsTab(resourcesToCheckable(data, [props.report()]));
		}
		const first = 0;
		const first_report = data?.[first];
		if (
			!props.clear() &&
			first_report &&
			props.branches().length === 0 &&
			props.testbeds().length === 0 &&
			props.benchmarks().length === 0 &&
			props.measures().length === 0 &&
			props.tab() === DEFAULT_PERF_TAB
		) {
			const benchmarks = first_report?.results?.[first]
				?.map((iteration) => iteration?.benchmark?.uuid)
				.slice(0, 10);
			const first_measure =
				first_report?.results?.[first]?.[first]?.measures?.[first]?.measure
					?.uuid;
			const start_time = dateTimeMillis(first_report?.start_time);
			props.setSearchParams(
				{
					[REPORT_PARAM]: first_report?.uuid,
					[BRANCHES_PARAM]: first_report?.branch?.uuid,
					[HEADS_PARAM]: first_report?.branch?.head?.uuid,
					[TESTBEDS_PARAM]: first_report?.testbed?.uuid,
					[BENCHMARKS_PARAM]: arrayToString(benchmarks ?? []),
					[MEASURES_PARAM]: first_measure,
					[PLOT_PARAM]: null,
					[START_TIME_PARAM]: start_time
						? start_time - DEFAULT_REPORT_HISTORY
						: null,
					[END_TIME_PARAM]: dateTimeMillis(first_report?.end_time),
					[LOWER_VALUE_PARAM]: null,
					[UPPER_VALUE_PARAM]: null,
					[LOWER_BOUNDARY_PARAM]:
						typeof first_measure?.boundary?.lower_limit === "number",
					[UPPER_BOUNDARY_PARAM]:
						typeof first_measure?.boundary?.upper_limit === "number",
					[CLEAR_PARAM]: true,
				},
				{ replace: true },
			);
		}
	});

	const branches_fetcher = createMemo(() => {
		return {
			project_slug: props.project_slug(),
			per_page: props.branches_per_page(),
			page: props.branches_page(),
			search: props.branches_search(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [branches_data] = createResource(branches_fetcher, async (fetcher) =>
		getPerfTab<JsonBranch>(PerfTab.BRANCHES, fetcher, (headers) =>
			setBranchesTotalCount(headers?.[X_TOTAL_COUNT]),
		),
	);
	createEffect(() => {
		const data = branches_data();
		if (data) {
			setBranchesTab(resourcesToCheckable(data, props.branches()));
		}
	});

	const testbeds_fetcher = createMemo(() => {
		return {
			project_slug: props.project_slug(),
			per_page: props.testbeds_per_page(),
			page: props.testbeds_page(),
			search: props.testbeds_search(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [testbeds_data] = createResource(testbeds_fetcher, async (fetcher) =>
		getPerfTab<JsonTestbed>(PerfTab.TESTBEDS, fetcher, (headers) =>
			setTestbedsTotalCount(headers?.[X_TOTAL_COUNT]),
		),
	);
	createEffect(() => {
		const data = testbeds_data();
		if (data) {
			setTestbedsTab(resourcesToCheckable(data, props.testbeds()));
		}
	});

	const benchmarks_fetcher = createMemo(() => {
		return {
			project_slug: props.project_slug(),
			per_page: props.benchmarks_per_page(),
			page: props.benchmarks_page(),
			search: props.benchmarks_search(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [benchmarks_data] = createResource(
		benchmarks_fetcher,
		async (fetcher) =>
			getPerfTab<JsonBenchmark>(PerfTab.BENCHMARKS, fetcher, (headers) =>
				setBenchmarksTotalCount(headers?.[X_TOTAL_COUNT]),
			),
	);
	createEffect(() => {
		const data = benchmarks_data();
		if (data) {
			setBenchmarksTab(resourcesToCheckable(data, props.benchmarks()));
		}
	});

	const plots_fetcher = createMemo(() => {
		return {
			project_slug: props.project_slug(),
			per_page: props.plots_per_page(),
			page: props.plots_page(),
			search: props.plots_search(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [plots_data] = createResource(plots_fetcher, async (fetcher) =>
		getPerfTab<JsonPlot>(PerfTab.PLOTS, fetcher, (headers) =>
			setPlotsTotalCount(headers?.[X_TOTAL_COUNT]),
		),
	);
	createEffect(() => {
		const data = plots_data();
		if (data) {
			setPlotsTab(resourcesToCheckable(data, [props.plot()]));
		}
	});

	const handleReportChecked = (index: number) => {
		const reportUuid = reports_tab?.[index]?.resource?.uuid;
		props.setSearchParams({
			[REPORT_PARAM]: props.report() === reportUuid ? null : reportUuid,
			[CLEAR_PARAM]: true,
		});
	};
	const handleChecked = (
		resource_tab: TabList<JsonBranch | JsonTestbed | JsonBenchmark>,
		index: undefined | number,
		param: string,
		param_array: string[],
		customParams?: (checked: boolean, i: null | number) => object,
	) => {
		// Uncheck all
		if (index === undefined) {
			props.setSearchParams({
				[REPORT_PARAM]: null,
				[PLOT_PARAM]: null,
				[param]: null,
				[CLEAR_PARAM]: true,
			});
			return;
		}
		const item = resource_tab?.[index];
		if (!item) {
			return;
		}
		const checked = item.checked;
		if (typeof checked !== "boolean") {
			return;
		}
		const uuid = item.resource.uuid;
		const [array, i] = checked
			? removeFromArray(param_array, uuid)
			: addToArray(param_array, uuid);
		props.setSearchParams({
			[REPORT_PARAM]: null,
			[PLOT_PARAM]: null,
			[param]: arrayToString(array),
			[CLEAR_PARAM]: true,
			...customParams?.(checked, i),
		});
	};
	const handleBranchChecked = (index: undefined | number) => {
		handleChecked(
			branches_tab,
			index,
			BRANCHES_PARAM,
			props.branches(),
			(checked, i) => {
				if (i === null) {
					return {};
				}
				const array = heads();
				if (checked) {
					array.splice(i, 1);
				} else {
					const head_uuid = branches_tab?.[index]?.resource?.head?.uuid;
					array.splice(i, 0, head_uuid);
				}
				return {
					[HEADS_PARAM]: arrayToString(array),
				};
			},
		);
	};
	const handleTestbedChecked = (index: undefined | number) => {
		handleChecked(testbeds_tab, index, TESTBEDS_PARAM, props.testbeds());
	};
	const handleBenchmarkChecked = (index: undefined | number) => {
		handleChecked(benchmarks_tab, index, BENCHMARKS_PARAM, props.benchmarks());
	};
	const handleMeasure = (measure: null | string) => {
		props.setSearchParams({
			[REPORT_PARAM]: null,
			[MEASURES_PARAM]: measure,
			[PLOT_PARAM]: null,
			[CLEAR_PARAM]: true,
		});
	};
	const handlePlotChecked = (index: number) => {
		const plot = plots_tab?.[index]?.resource;
		const now = Date.now();
		props.setSearchParams({
			[REPORT_PARAM]: null,
			[BRANCHES_PARAM]: plot?.branches?.join(","),
			[TESTBEDS_PARAM]: plot?.testbeds?.join(","),
			[BENCHMARKS_PARAM]: plot?.benchmarks?.join(","),
			[MEASURES_PARAM]: plot?.measures?.join(","),
			[PLOT_PARAM]: plot?.uuid,
			[START_TIME_PARAM]: now - (plot?.window ?? 1) * 1_000,
			[END_TIME_PARAM]: now,
			[LOWER_VALUE_PARAM]: plot?.lower_value,
			[UPPER_VALUE_PARAM]: plot?.upper_value,
			[LOWER_BOUNDARY_PARAM]: plot?.lower_boundary,
			[UPPER_BOUNDARY_PARAM]: plot?.upper_boundary,
			[CLEAR_PARAM]: true,
		});
	};

	const handleStartTime = (date: string) =>
		props.setSearchParams({
			[PLOT_PARAM]: null,
			[START_TIME_PARAM]: dateToTime(date),
		});
	const handleEndTime = (date: string) =>
		props.setSearchParams({
			[PLOT_PARAM]: null,
			[END_TIME_PARAM]: dateToTime(date),
		});

	const handleTab = (tab: PerfTab) => {
		if (isPerfTab(tab)) {
			props.setSearchParams({ [TAB_PARAM]: tab });
		}
	};

	const handleBool = (param: string, value: boolean) => {
		if (typeof value === "boolean") {
			props.setSearchParams({ [PLOT_PARAM]: null, [param]: value });
		}
	};

	const handleKey = (key: boolean) => {
		handleBool(KEY_PARAM, key);
	};

	const handleXAxis = (x_axis: XAxis) => {
		if (isXAxis(x_axis)) {
			props.setSearchParams({ [PLOT_PARAM]: null, [X_AXIS_PARAM]: x_axis });
		}
	};

	const handleClear = (clear: boolean) => {
		if (typeof clear === "boolean") {
			if (clear) {
				props.setSearchParams({
					[REPORT_PARAM]: null,
					[BRANCHES_PARAM]: null,
					[TESTBEDS_PARAM]: null,
					[BENCHMARKS_PARAM]: null,
					[MEASURES_PARAM]: null,
					[PLOT_PARAM]: null,
					[START_TIME_PARAM]: null,
					[END_TIME_PARAM]: null,
					[LOWER_VALUE_PARAM]: null,
					[UPPER_VALUE_PARAM]: null,
					[LOWER_BOUNDARY_PARAM]: null,
					[UPPER_BOUNDARY_PARAM]: null,
					[X_AXIS_PARAM]: null,
					[TAB_PARAM]: DEFAULT_PERF_TAB,
					[REPORTS_PER_PAGE_PARAM]: REPORTS_PER_PAGE,
					[BRANCHES_PER_PAGE_PARAM]: DEFAULT_PER_PAGE,
					[TESTBEDS_PER_PAGE_PARAM]: DEFAULT_PER_PAGE,
					[BENCHMARKS_PER_PAGE_PARAM]: DEFAULT_PER_PAGE,
					[PLOTS_PER_PAGE_PARAM]: DEFAULT_PER_PAGE,
					[REPORTS_PAGE_PARAM]: DEFAULT_PAGE,
					[BRANCHES_PAGE_PARAM]: DEFAULT_PAGE,
					[TESTBEDS_PAGE_PARAM]: DEFAULT_PAGE,
					[BENCHMARKS_PAGE_PARAM]: DEFAULT_PAGE,
					[PLOTS_PAGE_PARAM]: DEFAULT_PAGE,
					[REPORTS_START_TIME_PARAM]: null,
					[REPORTS_END_TIME_PARAM]: null,
					[BRANCHES_SEARCH_PARAM]: null,
					[TESTBEDS_SEARCH_PARAM]: null,
					[BENCHMARKS_SEARCH_PARAM]: null,
					[PLOTS_SEARCH_PARAM]: null,
					[EMBED_LOGO_PARAM]: null,
					[EMBED_TITLE_PARAM]: null,
					[EMBED_HEADER_PARAM]: null,
					[EMBED_KEY_PARAM]: null,
					[CLEAR_PARAM]: true,
				});
			} else {
				props.setSearchParams({ [CLEAR_PARAM]: clear });
			}
		}
	};

	const handleLowerValue = (end: boolean) => {
		handleBool(LOWER_VALUE_PARAM, end);
	};
	const handleUpperValue = (end: boolean) => {
		handleBool(UPPER_VALUE_PARAM, end);
	};

	const handleLowerBoundary = (boundary: boolean) => {
		handleBool(LOWER_BOUNDARY_PARAM, boundary);
	};
	const handleUpperBoundary = (boundary: boolean) => {
		handleBool(UPPER_BOUNDARY_PARAM, boundary);
	};

	const handleReportsPage = (page: number) =>
		props.setSearchParams({ [REPORTS_PAGE_PARAM]: page });
	const handleBranchesPage = (page: number) =>
		props.setSearchParams({ [BRANCHES_PAGE_PARAM]: page });
	const handleTestbedsPage = (page: number) =>
		props.setSearchParams({ [TESTBEDS_PAGE_PARAM]: page });
	const handleBenchmarksPage = (page: number) =>
		props.setSearchParams({ [BENCHMARKS_PAGE_PARAM]: page });
	const handlePlotsPage = (page: number) =>
		props.setSearchParams({ [PLOTS_PAGE_PARAM]: page });

	const handleReportsStartTime = (date: string) =>
		props.setSearchParams({
			[REPORTS_PAGE_PARAM]: DEFAULT_PAGE,
			[REPORTS_START_TIME_PARAM]: dateToTime(date),
		});
	const handleReportsEndTime = (date: string) =>
		props.setSearchParams({
			[REPORTS_PAGE_PARAM]: DEFAULT_PAGE,
			[REPORTS_END_TIME_PARAM]: dateToTime(date),
		});
	const handleBranchesSearch = debounce(
		(search: string) =>
			props.setSearchParams({
				[BRANCHES_PAGE_PARAM]: DEFAULT_PAGE,
				[BRANCHES_SEARCH_PARAM]: search,
			}),
		DEBOUNCE_DELAY,
	);
	const handleTestbedsSearch = debounce(
		(search: string) =>
			props.setSearchParams({
				[TESTBEDS_PAGE_PARAM]: DEFAULT_PAGE,
				[TESTBEDS_SEARCH_PARAM]: search,
			}),
		DEBOUNCE_DELAY,
	);
	const handleBenchmarksSearch = debounce(
		(search: string) =>
			props.setSearchParams({
				[BENCHMARKS_PAGE_PARAM]: DEFAULT_PAGE,
				[BENCHMARKS_SEARCH_PARAM]: search,
			}),
		DEBOUNCE_DELAY,
	);
	const handlePlotsSearch = debounce(
		(search: string) =>
			props.setSearchParams({
				[PLOTS_PAGE_PARAM]: DEFAULT_PAGE,
				[PLOTS_SEARCH_PARAM]: search,
			}),
		DEBOUNCE_DELAY,
	);

	return (
		<>
			<Show when={!props.isEmbed}>
				<PerfHeader
					isConsole={props.isConsole === true}
					apiUrl={props.apiUrl}
					user={user}
					project={project}
					isPlotInit={isPlotInit}
					perfQuery={perfQuery}
					lower_value={props.lower_value}
					upper_value={props.upper_value}
					lower_boundary={props.lower_boundary}
					upper_boundary={props.upper_boundary}
					x_axis={props.x_axis}
					branches={props.branches}
					testbeds={props.testbeds}
					benchmarks={props.benchmarks}
					measures={props.measures}
					plot={props.plot}
					handleRefresh={handleRefresh}
				/>
			</Show>
			<PerfPlot
				apiUrl={props.apiUrl}
				user={user}
				project={project}
				project_slug={props.project_slug}
				theme={theme}
				isConsole={props.isConsole === true}
				isEmbed={props.isEmbed === true}
				isPlotInit={isPlotInit}
				report={props.report}
				measures={props.measures}
				branches={props.branches}
				testbeds={props.testbeds}
				benchmarks={props.benchmarks}
				start_date={props.start_date}
				end_date={props.end_date}
				refresh={refresh}
				perfData={perfData}
				tab={props.tab}
				key={props.key}
				x_axis={props.x_axis}
				clear={props.clear}
				lower_value={props.lower_value}
				upper_value={props.upper_value}
				lower_boundary={props.lower_boundary}
				upper_boundary={props.upper_boundary}
				reports_data={reports_data}
				branches_data={branches_data}
				testbeds_data={testbeds_data}
				benchmarks_data={benchmarks_data}
				plots_data={plots_data}
				reports_tab={reports_tab}
				branches_tab={branches_tab}
				testbeds_tab={testbeds_tab}
				benchmarks_tab={benchmarks_tab}
				plots_tab={plots_tab}
				reports_per_page={props.reports_per_page}
				branches_per_page={props.branches_per_page}
				testbeds_per_page={props.testbeds_per_page}
				benchmarks_per_page={props.benchmarks_per_page}
				plots_per_page={props.plots_per_page}
				reports_page={props.reports_page}
				branches_page={props.branches_page}
				testbeds_page={props.testbeds_page}
				benchmarks_page={props.benchmarks_page}
				plots_page={props.plots_page}
				reports_total_count={reportsTotalCount}
				branches_total_count={branchesTotalCount}
				testbeds_total_count={testbedsTotalCount}
				benchmarks_total_count={benchmarksTotalCount}
				plots_total_count={plotsTotalCount}
				reports_start_date={props.reports_start_date}
				reports_end_date={props.reports_end_date}
				branches_search={props.branches_search}
				testbeds_search={props.testbeds_search}
				benchmarks_search={props.benchmarks_search}
				plots_search={props.plots_search}
				embed_logo={props.embed_logo}
				embed_title={props.embed_title}
				embed_header={props.embed_header}
				embed_key={props.embed_key}
				handleMeasure={handleMeasure}
				handleStartTime={handleStartTime}
				handleEndTime={handleEndTime}
				handleTab={handleTab}
				handleKey={handleKey}
				handleXAxis={handleXAxis}
				handleClear={handleClear}
				handleLowerValue={handleLowerValue}
				handleUpperValue={handleUpperValue}
				handleLowerBoundary={handleLowerBoundary}
				handleUpperBoundary={handleUpperBoundary}
				handleReportChecked={handleReportChecked}
				handleBranchChecked={handleBranchChecked}
				handleTestbedChecked={handleTestbedChecked}
				handleBenchmarkChecked={handleBenchmarkChecked}
				handlePlotChecked={handlePlotChecked}
				handleReportsPage={handleReportsPage}
				handleBranchesPage={handleBranchesPage}
				handleTestbedsPage={handleTestbedsPage}
				handleBenchmarksPage={handleBenchmarksPage}
				handlePlotsPage={handlePlotsPage}
				handleReportsStartTime={handleReportsStartTime}
				handleReportsEndTime={handleReportsEndTime}
				handleBranchesSearch={handleBranchesSearch}
				handleTestbedsSearch={handleTestbedsSearch}
				handleBenchmarksSearch={handleBenchmarksSearch}
				handlePlotsSearch={handlePlotsSearch}
			/>
		</>
	);
};

export default PerfFrame;
