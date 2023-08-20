import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import PerfHeader, { PerfQuery } from "./PerfHeader";
import PerfPlot from "./plot/PerfPlot";
import { createStore } from "solid-js/store";
import {
	PerfRange,
	PerfTab,
	isPerfRange,
	isPerfTab,
} from "../../../config/types";
import { useSearchParams } from "../../../util/url";
import { validSlug, validU32 } from "../../../util/valid";
import type { Params } from "astro";
import { authUser } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import type {
	JsonBenchmark,
	JsonBranch,
	JsonPerf,
	JsonReport,
	JsonTestbed,
} from "../../../types/bencher";
import type { TabList } from "./plot/PlotTab";
import { BENCHER_API_URL } from "../../../util/ext";
import { NotifyKind, pageNotify } from "../../../util/notify";

const REPORT_PARAM = "report";
const METRIC_KIND_PARAM = "metric_kind";
const BRANCHES_PARAM = "branches";
const TESTBEDS_PARAM = "testbeds";
const BENCHMARKS_PARAM = "benchmarks";
const START_TIME_PARAM = "start_time";
const END_TIME_PARAM = "end_time";

const REPORTS_PER_PAGE_PARAM = "reports_per_page";
const BRANCHES_PER_PAGE_PARAM = "branches_per_page";
const TESTBEDS_PER_PAGE_PARAM = "testbeds_per_page";
const BENCHMARKS_PER_PAGE_PARAM = "benchmarks_per_page";

const REPORTS_PAGE_PARAM = "reports_page";
const BRANCHES_PAGE_PARAM = "branches_page";
const TESTBEDS_PAGE_PARAM = "testbeds_page";
const BENCHMARKS_PAGE_PARAM = "benchmarks_page";

const TAB_PARAM = "tab";
const KEY_PARAM = "key";
const RANGE_PARAM = "range";
const CLEAR_PARAM = "clear";
const LOWER_BOUNDARY_PARAM = "lower_boundary";
const UPPER_BOUNDARY_PARAM = "upper_boundary";

const DEFAULT_PERF_TAB = PerfTab.REPORTS;
const DEFAULT_PERF_KEY = true;
const DEFAULT_PERF_RANGE = PerfRange.DATE_TIME;
const DEFAULT_PERF_CLEAR = false;
const DEFAULT_PERF_BOUNDARY = false;

const DEFAULT_PER_PAGE = 8;
const REPORTS_PER_PAGE = 4;
export const DEFAULT_PAGE = 1;

const addToArray = (array: any[], add: any): string[] => {
	if (!array.includes(add)) {
		array.push(add);
	}
	return array;
};
const removeFromArray = (array: any[], remove: any): string[] => {
	const index = array.indexOf(remove);
	if (index > -1) {
		array.splice(index, 1);
	}
	return array;
};

const arrayFromString = (array_str: undefined | string): string[] => {
	if (typeof array_str === "string") {
		const array = array_str.split(",");
		return removeFromArray(array, "");
	}
	return [];
};
const arrayToString = (array: any[]) => array.join();

const timeToDate = (time_str: undefined | string): null | Date => {
	if (typeof time_str === "string") {
		const time = parseInt(time_str);
		if (Number.isInteger(time)) {
			const date = new Date(time);
			if (date) {
				return date;
			}
		}
	}
	return null;
};

const timeToDateIso = (time_str: undefined | string): null | string => {
	const date = timeToDate(time_str);
	if (date) {
		return date.toISOString();
	}
	return null;
};

const timeToDateOnlyIso = (
	time_str: undefined | string,
): undefined | string => {
	const iso_date = timeToDateIso(time_str);
	if (iso_date) {
		return iso_date.split("T")?.[0];
	}
	return;
};

const dateToTime = (date_str: undefined | string): null | string => {
	if (typeof date_str === "string") {
		const time = Date.parse(date_str);
		if (time) {
			return `${time}`;
		}
	}
	return null;
};

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

const isBoolParam = (param: undefined | string): boolean => {
	return param === "false" || param === "true";
};

export interface Props {
	apiUrl: string;
	params: Params;
	isConsole?: boolean;
}

const PerfPanel = (props: Props) => {
	const params = createMemo(() => props.params);
	const [searchParams, setSearchParams] = useSearchParams();
	const user = authUser();

	// Sanitize all query params at init
	const initParams: Record<string, null | number | boolean> = {};
	if (typeof searchParams[METRIC_KIND_PARAM] !== "string") {
		initParams[METRIC_KIND_PARAM] = null;
	}
	if (!Array.isArray(arrayFromString(searchParams[BRANCHES_PARAM]))) {
		initParams[BRANCHES_PARAM] = null;
	}
	if (!Array.isArray(arrayFromString(searchParams[TESTBEDS_PARAM]))) {
		initParams[TESTBEDS_PARAM] = null;
	}
	if (!Array.isArray(arrayFromString(searchParams[BENCHMARKS_PARAM]))) {
		initParams[BENCHMARKS_PARAM] = null;
	}
	if (!timeToDate(searchParams[START_TIME_PARAM])) {
		initParams[START_TIME_PARAM] = null;
	}
	if (!timeToDate(searchParams[END_TIME_PARAM])) {
		initParams[END_TIME_PARAM] = null;
	}

	// Sanitize all UI state query params
	if (!isPerfTab(searchParams[TAB_PARAM])) {
		initParams[TAB_PARAM] = null;
	}
	if (!isBoolParam(searchParams[KEY_PARAM])) {
		initParams[KEY_PARAM] = DEFAULT_PERF_KEY;
	}
	if (!isPerfRange(searchParams[RANGE_PARAM])) {
		initParams[RANGE_PARAM] = null;
	}
	if (!isBoolParam(searchParams[CLEAR_PARAM])) {
		initParams[CLEAR_PARAM] = null;
	}
	if (!isBoolParam(searchParams[LOWER_BOUNDARY_PARAM])) {
		initParams[LOWER_BOUNDARY_PARAM] = null;
	}
	if (!isBoolParam(searchParams[UPPER_BOUNDARY_PARAM])) {
		initParams[UPPER_BOUNDARY_PARAM] = null;
	}

	// Sanitize all pagination query params
	if (!validU32(searchParams[REPORTS_PER_PAGE_PARAM])) {
		initParams[REPORTS_PER_PAGE_PARAM] = REPORTS_PER_PAGE;
	}
	if (!validU32(searchParams[BRANCHES_PER_PAGE_PARAM])) {
		initParams[BRANCHES_PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
	}
	if (!validU32(searchParams[TESTBEDS_PER_PAGE_PARAM])) {
		initParams[TESTBEDS_PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
	}
	if (!validU32(searchParams[BENCHMARKS_PER_PAGE_PARAM])) {
		initParams[BENCHMARKS_PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
	}

	if (!validU32(searchParams[REPORTS_PAGE_PARAM])) {
		initParams[REPORTS_PAGE_PARAM] = DEFAULT_PAGE;
	}
	if (!validU32(searchParams[BRANCHES_PAGE_PARAM])) {
		initParams[BRANCHES_PAGE_PARAM] = DEFAULT_PAGE;
	}
	if (!validU32(searchParams[TESTBEDS_PAGE_PARAM])) {
		initParams[TESTBEDS_PAGE_PARAM] = DEFAULT_PAGE;
	}
	if (!validU32(searchParams[BENCHMARKS_PAGE_PARAM])) {
		initParams[BENCHMARKS_PAGE_PARAM] = DEFAULT_PAGE;
	}
	if (Object.keys(initParams).length !== 0) {
		setSearchParams(initParams);
	}

	// Create marshalized memos of all query params
	const metric_kind = createMemo(() => searchParams[METRIC_KIND_PARAM]);
	const report = createMemo(() => searchParams[REPORT_PARAM]);
	const branches = createMemo(() =>
		arrayFromString(searchParams[BRANCHES_PARAM]),
	);
	const testbeds = createMemo(() =>
		arrayFromString(searchParams[TESTBEDS_PARAM]),
	);
	const benchmarks = createMemo(() =>
		arrayFromString(searchParams[BENCHMARKS_PARAM]),
	);
	// start/end_time is used for the query
	const start_time = createMemo(() => searchParams[START_TIME_PARAM]);
	const end_time = createMemo(() => searchParams[END_TIME_PARAM]);
	// start/end_date is used for the GUI selector
	const start_date = createMemo(() => timeToDateOnlyIso(start_time()));
	const end_date = createMemo(() => timeToDateOnlyIso(end_time()));

	const tab = createMemo(() => {
		// This check is required for the initial load
		// before the query params have been sanitized
		const perfTab = searchParams[TAB_PARAM];
		if (perfTab && isPerfTab(perfTab)) {
			return perfTab as PerfTab;
		} else {
			return DEFAULT_PERF_TAB;
		}
	});

	// This check is required for the initial load
	// before the query params have been sanitized
	const isBoolParamOrDefault = (param: string, default_value: boolean) => {
		if (isBoolParam(searchParams[param])) {
			return searchParams[param] === "true";
		} else {
			return default_value;
		}
	};

	const key = createMemo(() =>
		isBoolParamOrDefault(KEY_PARAM, DEFAULT_PERF_KEY),
	);

	const range = createMemo(() => {
		// This check is required for the initial load
		// before the query params have been sanitized
		const perfRange = searchParams[RANGE_PARAM];
		if (perfRange && isPerfRange(perfRange)) {
			return perfRange as PerfRange;
		} else {
			return DEFAULT_PERF_RANGE;
		}
	});

	// Ironically, a better name for the `clear` param would actually be `dirty`.
	// It works as a "dirty" bit to indicate that we shouldn't load the first report again.
	const clear = createMemo(() =>
		isBoolParamOrDefault(CLEAR_PARAM, DEFAULT_PERF_CLEAR),
	);

	const lower_boundary = createMemo(() =>
		isBoolParamOrDefault(LOWER_BOUNDARY_PARAM, DEFAULT_PERF_BOUNDARY),
	);
	const upper_boundary = createMemo(() =>
		isBoolParamOrDefault(UPPER_BOUNDARY_PARAM, DEFAULT_PERF_BOUNDARY),
	);

	// Pagination query params
	const reports_per_page = createMemo(() =>
		Number(searchParams[REPORTS_PER_PAGE_PARAM]),
	);
	const branches_per_page = createMemo(() =>
		Number(searchParams[BRANCHES_PER_PAGE_PARAM]),
	);
	const testbeds_per_page = createMemo(() =>
		Number(searchParams[TESTBEDS_PER_PAGE_PARAM]),
	);
	const benchmarks_per_page = createMemo(() =>
		Number(searchParams[BENCHMARKS_PER_PAGE_PARAM]),
	);

	const reports_page = createMemo(() =>
		Number(searchParams[REPORTS_PAGE_PARAM]),
	);
	const branches_page = createMemo(() =>
		Number(searchParams[BRANCHES_PAGE_PARAM]),
	);
	const testbeds_page = createMemo(() =>
		Number(searchParams[TESTBEDS_PAGE_PARAM]),
	);
	const benchmarks_page = createMemo(() =>
		Number(searchParams[BENCHMARKS_PAGE_PARAM]),
	);

	// The perf query sent to the server
	const perfQuery = createMemo(() => {
		return {
			metric_kind: metric_kind(),
			branches: branches(),
			testbeds: testbeds(),
			benchmarks: benchmarks(),
			start_time: start_time(),
			end_time: end_time(),
		};
	});

	const isPlotInit = createMemo(
		() =>
			!metric_kind() ||
			branches().length === 0 ||
			testbeds().length === 0 ||
			benchmarks().length === 0,
	);

	// Refresh pref query
	const [refresh, setRefresh] = createSignal(0);
	const handleRefresh = () => {
		setRefresh(refresh() + 1);
	};

	const project_slug = createMemo(() => params().project);
	const perfFetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			perfQuery: perfQuery(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const getPerf = async (fetcher: {
		project_slug: string;
		perfQuery: PerfQuery;
		refresh: number;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		if (props.isConsole && typeof fetcher.token !== "string") {
			return EMPTY_OBJECT;
		}

		// Don't even send query if there isn't at least one: branch, testbed, and benchmark
		if (isPlotInit()) {
			const path = `/v0/projects/${fetcher.project_slug}`;
			return await httpGet(props.apiUrl, path, fetcher.token)
				.then((resp) => {
					return {
						project: resp?.data,
					} as JsonPerf;
				})
				.catch((error) => {
					console.error(error);
					return EMPTY_OBJECT;
				});
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
				pageNotify(
					NotifyKind.ERROR,
					"Lettuce romaine calm! Failed to get perf. Please, try again.",
				);
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

	// Resource tabs data: Reports, Branches, Testbeds, Benchmarks
	async function getPerfTab<T>(
		perfTab: PerfTab,
		fetcher: {
			project_slug: undefined | string;
			per_page: number;
			page: number;
			token: string;
		},
	) {
		const EMPTY_ARRAY: T[] = [];
		if (!fetcher.project_slug) {
			return EMPTY_ARRAY;
		}
		if (props.isConsole && typeof fetcher.token !== "string") {
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
		const path = `/v0/projects/${
			fetcher.project_slug
		}/${perfTab}?${search_params.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return EMPTY_ARRAY;
			});
	}
	createEffect(() => {
		const newParams: Record<string, number> = {};
		if (!validU32(searchParams[REPORTS_PER_PAGE_PARAM])) {
			newParams[REPORTS_PER_PAGE_PARAM] = REPORTS_PER_PAGE;
		}
		if (!validU32(searchParams[REPORTS_PAGE_PARAM])) {
			newParams[REPORTS_PAGE_PARAM] = DEFAULT_PAGE;
		}
		if (!validU32(searchParams[BRANCHES_PER_PAGE_PARAM])) {
			newParams[BRANCHES_PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
		}
		if (!validU32(searchParams[BRANCHES_PAGE_PARAM])) {
			newParams[BRANCHES_PAGE_PARAM] = DEFAULT_PAGE;
		}
		if (!validU32(searchParams[TESTBEDS_PER_PAGE_PARAM])) {
			newParams[TESTBEDS_PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
		}
		if (!validU32(searchParams[TESTBEDS_PAGE_PARAM])) {
			newParams[TESTBEDS_PAGE_PARAM] = DEFAULT_PAGE;
		}
		if (!validU32(searchParams[BENCHMARKS_PER_PAGE_PARAM])) {
			newParams[BENCHMARKS_PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
		}
		if (!validU32(searchParams[BENCHMARKS_PAGE_PARAM])) {
			newParams[BENCHMARKS_PAGE_PARAM] = DEFAULT_PAGE;
		}
		if (Object.keys(newParams).length !== 0) {
			setSearchParams(newParams);
		}
	});

	const reports_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			per_page: reports_per_page(),
			page: reports_page(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [reports_data] = createResource(reports_fetcher, async (fetcher) =>
		getPerfTab<JsonReport>(PerfTab.REPORTS, fetcher),
	);
	createEffect(() => {
		const data = reports_data();
		if (data) {
			setReportsTab(resourcesToCheckable(data, [report()]));
		}
		const first = 0;
		const first_report = data?.[first];
		if (
			!clear() &&
			first_report &&
			!metric_kind() &&
			branches().length === 0 &&
			testbeds().length === 0 &&
			benchmarks().length === 0 &&
			tab() === DEFAULT_PERF_TAB
		) {
			const first_metric_kind =
				first_report?.results?.[first]?.[first].metric_kind?.slug;
			handleReportChecked(first, first_metric_kind);
		}
	});

	const branches_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			per_page: branches_per_page(),
			page: branches_page(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [branches_data] = createResource(branches_fetcher, async (fetcher) =>
		getPerfTab<JsonBranch>(PerfTab.BRANCHES, fetcher),
	);
	createEffect(() => {
		const data = branches_data();
		if (data) {
			setBranchesTab(resourcesToCheckable(data, branches()));
		}
	});

	const testbeds_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			per_page: testbeds_per_page(),
			page: testbeds_page(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [testbeds_data] = createResource(testbeds_fetcher, async (fetcher) =>
		getPerfTab<JsonTestbed>(PerfTab.TESTBEDS, fetcher),
	);
	createEffect(() => {
		const data = testbeds_data();
		if (data) {
			setTestbedsTab(resourcesToCheckable(data, testbeds()));
		}
	});

	const benchmarks_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			per_page: benchmarks_per_page(),
			page: benchmarks_page(),
			refresh: refresh(),
			token: user?.token,
		};
	});
	const [benchmarks_data] = createResource(
		benchmarks_fetcher,
		async (fetcher) => getPerfTab<JsonBenchmark>(PerfTab.BENCHMARKS, fetcher),
	);
	createEffect(() => {
		const data = benchmarks_data();
		if (data) {
			setBenchmarksTab(resourcesToCheckable(data, benchmarks()));
		}
	});

	const handleMetricKind = (metric_kind: null | string) => {
		setSearchParams({
			[CLEAR_PARAM]: true,
			[REPORT_PARAM]: null,
			[METRIC_KIND_PARAM]: validSlug(metric_kind) ? metric_kind : null,
		});
	};

	const handleReportChecked = (
		index: number,
		metric_kind_slug: undefined | string,
	) => {
		if (!metric_kind_slug) {
			return;
		}
		const report = reports_tab?.[index]?.resource;
		const benchmarks = report?.results?.[0]
			?.find((result) => result.metric_kind?.slug === metric_kind_slug)
			?.benchmarks?.map((benchmark) => benchmark.uuid);
		setSearchParams({
			[CLEAR_PARAM]: true,
			[LOWER_BOUNDARY_PARAM]: null,
			[UPPER_BOUNDARY_PARAM]: null,
			[REPORT_PARAM]: report?.uuid,
			[METRIC_KIND_PARAM]: metric_kind_slug,
			[BRANCHES_PARAM]: report?.branch?.uuid,
			[TESTBEDS_PARAM]: report?.testbed?.uuid,
			[BENCHMARKS_PARAM]: arrayToString(benchmarks ?? []),
		});
	};
	const handleChecked = (
		resource_tab: any[],
		index: number,
		param: string,
		param_array: string[],
	) => {
		const item = resource_tab?.[index];
		const checked = item.checked;
		if (typeof checked !== "boolean") {
			return;
		}
		const uuid = item.resource.uuid;
		if (checked) {
			setSearchParams({
				[CLEAR_PARAM]: true,
				[REPORT_PARAM]: null,
				[param]: arrayToString(removeFromArray(param_array, uuid)),
			});
		} else {
			setSearchParams({
				[CLEAR_PARAM]: true,
				[REPORT_PARAM]: null,
				[param]: arrayToString(addToArray(param_array, uuid)),
			});
		}
	};
	const handleBranchChecked = (index: number) => {
		handleChecked(branches_tab, index, BRANCHES_PARAM, branches());
	};
	const handleTestbedChecked = (index: number) => {
		handleChecked(testbeds_tab, index, TESTBEDS_PARAM, testbeds());
	};
	const handleBenchmarkChecked = (index: number) => {
		handleChecked(benchmarks_tab, index, BENCHMARKS_PARAM, benchmarks());
	};

	const handleStartTime = (date: string) =>
		setSearchParams({ [START_TIME_PARAM]: dateToTime(date) });
	const handleEndTime = (date: string) =>
		setSearchParams({ [END_TIME_PARAM]: dateToTime(date) });

	const handleTab = (tab: PerfTab) => {
		if (isPerfTab(tab)) {
			setSearchParams({ [TAB_PARAM]: tab });
		}
	};

	const handleBool = (param: string, value: boolean) => {
		if (typeof value === "boolean") {
			setSearchParams({ [param]: value });
		}
	};

	const handleKey = (key: boolean) => {
		handleBool(KEY_PARAM, key);
	};

	const handleRange = (range: PerfRange) => {
		if (isPerfRange(range)) {
			setSearchParams({ [RANGE_PARAM]: range });
		}
	};

	const handleClear = (clear: boolean) => {
		if (typeof clear === "boolean") {
			if (clear) {
				setSearchParams({
					[CLEAR_PARAM]: true,
					[LOWER_BOUNDARY_PARAM]: null,
					[UPPER_BOUNDARY_PARAM]: null,
					[REPORT_PARAM]: null,
					[METRIC_KIND_PARAM]: null,
					[BRANCHES_PARAM]: null,
					[TESTBEDS_PARAM]: null,
					[BENCHMARKS_PARAM]: null,
					[START_TIME_PARAM]: null,
					[END_TIME_PARAM]: null,
				});
			} else {
				setSearchParams({ [CLEAR_PARAM]: clear });
			}
		}
	};

	const handleLowerBoundary = (boundary: boolean) => {
		handleBool(LOWER_BOUNDARY_PARAM, boundary);
	};
	const handleUpperBoundary = (boundary: boolean) => {
		handleBool(UPPER_BOUNDARY_PARAM, boundary);
	};

	const handleReportsPage = (page: number) =>
		setSearchParams({ [REPORTS_PAGE_PARAM]: page });
	const handleBranchesPage = (page: number) =>
		setSearchParams({ [BRANCHES_PAGE_PARAM]: page });
	const handleTestbedsPage = (page: number) =>
		setSearchParams({ [TESTBEDS_PAGE_PARAM]: page });
	const handleBenchmarksPage = (page: number) =>
		setSearchParams({ [BENCHMARKS_PAGE_PARAM]: page });

	return (
		<>
			<PerfHeader
				user={user}
				perfData={perfData}
				isPlotInit={isPlotInit}
				perfQuery={perfQuery}
				handleRefresh={handleRefresh}
			/>
			<PerfPlot
				user={user}
				project_slug={project_slug}
				isConsole={props.isConsole ?? false}
				isPlotInit={isPlotInit}
				metric_kind={metric_kind}
				report={report}
				branches={branches}
				testbeds={testbeds}
				benchmarks={benchmarks}
				start_date={start_date}
				end_date={end_date}
				refresh={refresh}
				perfData={perfData}
				tab={tab}
				key={key}
				range={range}
				clear={clear}
				lower_boundary={lower_boundary}
				upper_boundary={upper_boundary}
				reports_tab={reports_tab}
				branches_tab={branches_tab}
				testbeds_tab={testbeds_tab}
				benchmarks_tab={benchmarks_tab}
				reports_per_page={reports_per_page}
				branches_per_page={branches_per_page}
				testbeds_per_page={testbeds_per_page}
				benchmarks_per_page={benchmarks_per_page}
				reports_page={reports_page}
				branches_page={branches_page}
				testbeds_page={testbeds_page}
				benchmarks_page={benchmarks_page}
				handleMetricKind={handleMetricKind}
				handleStartTime={handleStartTime}
				handleEndTime={handleEndTime}
				handleTab={handleTab}
				handleKey={handleKey}
				handleRange={handleRange}
				handleClear={handleClear}
				handleLowerBoundary={handleLowerBoundary}
				handleUpperBoundary={handleUpperBoundary}
				handleReportChecked={handleReportChecked}
				handleBranchChecked={handleBranchChecked}
				handleTestbedChecked={handleTestbedChecked}
				handleBenchmarkChecked={handleBenchmarkChecked}
				handleReportsPage={handleReportsPage}
				handleBranchesPage={handleBranchesPage}
				handleTestbedsPage={handleTestbedsPage}
				handleBenchmarksPage={handleBenchmarksPage}
			/>
		</>
	);
};

export default PerfPanel;
