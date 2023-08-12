import axios from "axios";
import { useSearchParams } from "solid-app-router";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { is_perf_tab, is_range, PerfTab, Range } from "../../config/types";
import { is_valid_slug } from "bencher_valid";
import {
	get_options,
	validate_jwt,
	validate_string,
	validate_u32,
} from "../../../site/util";
import PerfHeader from "./PerfHeader";
import PerfPlot from "./plot/PerfPlot";
import { createStore } from "solid-js/store";

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
const DEFAULT_PERF_RANGE = Range.DATE_TIME;
const DEFAULT_PERF_CLEAR = false;
const DEFAULT_PERF_BOUNDARY = false;

const DEFAULT_PER_PAGE = 8;
const REPORTS_PER_PAGE = 4;
export const DEFAULT_PAGE = 1;

const addToArray = (array: any[], add: any) => {
	if (!array.includes(add)) {
		array.push(add);
	}
	return array;
};
const removeFromArray = (array: any[], remove: any) => {
	const index = array.indexOf(remove);
	if (index > -1) {
		array.splice(index, 1);
	}
	return array;
};

const arrayFromString = (array_str: undefined | string) => {
	if (typeof array_str === "string") {
		const array = array_str.split(",");
		return removeFromArray(array, "");
	}
	return [];
};
const arrayToString = (array: any[]) => array.join();

const time_to_date = (time_str: undefined | string): null | Date => {
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

const time_to_date_iso = (time_str: undefined | string): null | string => {
	const date = time_to_date(time_str);
	if (date) {
		return date.toISOString();
	}
	return null;
};

const time_to_date_only_iso = (time_str: undefined | string): null | string => {
	const iso_date = time_to_date_iso(time_str);
	if (iso_date) {
		return iso_date.split("T")?.[0];
	}
	return null;
};

const date_to_time = (date_str: undefined | string): null | string => {
	if (typeof date_str === "string") {
		const time = Date.parse(date_str);
		if (time) {
			return `${time}`;
		}
	}
	return null;
};

const resourcesToCheckable = (resources, params) =>
	resources.map((resource) => {
		return {
			resource: resource,
			checked: params.includes(resource?.uuid),
		};
	});

const is_bool_param = (param: string): boolean => {
	return param === "false" || param === "true";
};

const PerfPanel = (props) => {
	const [searchParams, setSearchParams] = useSearchParams();

	// Sanitize all query params at init
	if (!validate_string(searchParams[METRIC_KIND_PARAM], is_valid_slug)) {
		setSearchParams({ [METRIC_KIND_PARAM]: null });
	}
	if (!Array.isArray(arrayFromString(searchParams[BRANCHES_PARAM]))) {
		setSearchParams({ [BRANCHES_PARAM]: null });
	}
	if (!Array.isArray(arrayFromString(searchParams[TESTBEDS_PARAM]))) {
		setSearchParams({ [TESTBEDS_PARAM]: null });
	}
	if (!Array.isArray(arrayFromString(searchParams[BENCHMARKS_PARAM]))) {
		setSearchParams({ [BENCHMARKS_PARAM]: null });
	}
	if (!time_to_date(searchParams[START_TIME_PARAM])) {
		setSearchParams({ [START_TIME_PARAM]: null });
	}
	if (!time_to_date(searchParams[END_TIME_PARAM])) {
		setSearchParams({ [END_TIME_PARAM]: null });
	}

	// Sanitize all UI state query params
	if (!is_perf_tab(searchParams[TAB_PARAM])) {
		setSearchParams({ [TAB_PARAM]: null });
	}
	if (!is_bool_param(searchParams[KEY_PARAM])) {
		setSearchParams({ [KEY_PARAM]: DEFAULT_PERF_KEY });
	}
	if (!is_range(searchParams[RANGE_PARAM])) {
		setSearchParams({ [RANGE_PARAM]: null });
	}
	if (!is_bool_param(searchParams[CLEAR_PARAM])) {
		setSearchParams({ [CLEAR_PARAM]: null });
	}
	if (!is_bool_param(searchParams[LOWER_BOUNDARY_PARAM])) {
		setSearchParams({ [LOWER_BOUNDARY_PARAM]: null });
	}
	if (!is_bool_param(searchParams[UPPER_BOUNDARY_PARAM])) {
		setSearchParams({ [UPPER_BOUNDARY_PARAM]: null });
	}

	// Sanitize all pagination query params
	if (!validate_u32(searchParams[REPORTS_PER_PAGE_PARAM])) {
		setSearchParams({ [REPORTS_PER_PAGE_PARAM]: REPORTS_PER_PAGE });
	}
	if (!validate_u32(searchParams[BRANCHES_PER_PAGE_PARAM])) {
		setSearchParams({ [BRANCHES_PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
	}
	if (!validate_u32(searchParams[TESTBEDS_PER_PAGE_PARAM])) {
		setSearchParams({ [TESTBEDS_PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
	}
	if (!validate_u32(searchParams[BENCHMARKS_PER_PAGE_PARAM])) {
		setSearchParams({ [BENCHMARKS_PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
	}

	if (!validate_u32(searchParams[REPORTS_PAGE_PARAM])) {
		setSearchParams({ [REPORTS_PAGE_PARAM]: DEFAULT_PAGE });
	}
	if (!validate_u32(searchParams[BRANCHES_PAGE_PARAM])) {
		setSearchParams({ [BRANCHES_PAGE_PARAM]: DEFAULT_PAGE });
	}
	if (!validate_u32(searchParams[TESTBEDS_PAGE_PARAM])) {
		setSearchParams({ [TESTBEDS_PAGE_PARAM]: DEFAULT_PAGE });
	}
	if (!validate_u32(searchParams[BENCHMARKS_PAGE_PARAM])) {
		setSearchParams({ [BENCHMARKS_PAGE_PARAM]: DEFAULT_PAGE });
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
	const start_date = createMemo(() => time_to_date_only_iso(start_time()));
	const end_date = createMemo(() => time_to_date_only_iso(end_time()));

	const tab = createMemo(() => {
		// This check is required for the initial load
		// before the query params have been sanitized
		if (is_perf_tab(searchParams[TAB_PARAM])) {
			return searchParams[TAB_PARAM];
		} else {
			return DEFAULT_PERF_TAB;
		}
	});

	// This check is required for the initial load
	// before the query params have been sanitized
	const is_bool_param_or_default = (param: string, default_value: boolean) => {
		if (is_bool_param(searchParams[param])) {
			return searchParams[param] === "true";
		} else {
			return default_value;
		}
	};

	const key = createMemo(() =>
		is_bool_param_or_default(KEY_PARAM, DEFAULT_PERF_KEY),
	);

	const range = createMemo(() => {
		// This check is required for the initial load
		// before the query params have been sanitized
		if (is_range(searchParams[RANGE_PARAM])) {
			return searchParams[RANGE_PARAM];
		} else {
			return DEFAULT_PERF_RANGE;
		}
	});

	// Ironically, a better name for the `clear` param would actually be `dirty`.
	// It works as a "dirty" bit to indicate that we shouldn't load the first report again.
	const clear = createMemo(() =>
		is_bool_param_or_default(CLEAR_PARAM, DEFAULT_PERF_CLEAR),
	);

	const lower_boundary = createMemo(() =>
		is_bool_param_or_default(LOWER_BOUNDARY_PARAM, DEFAULT_PERF_BOUNDARY),
	);
	const upper_boundary = createMemo(() =>
		is_bool_param_or_default(UPPER_BOUNDARY_PARAM, DEFAULT_PERF_BOUNDARY),
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
	const perf_query = createMemo(() => {
		return {
			metric_kind: metric_kind(),
			branches: branches(),
			testbeds: testbeds(),
			benchmarks: benchmarks(),
			start_time: start_time(),
			end_time: end_time(),
		};
	});

	const is_plot_init = createMemo(
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

	const project_slug = createMemo(() => props.path_params?.project_slug);
	const perf_query_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			perf_query: perf_query(),
			refresh: refresh(),
			token: props.user?.token,
		};
	});

	const get_perf = async (fetcher) => {
		const EMPTY_OBJECT = {};
		if (props.is_console && !validate_jwt(fetcher.token)) {
			return EMPTY_OBJECT;
		}
		// Don't even send query if there isn't at least one: branch, testbed, and benchmark
		if (is_plot_init()) {
			const url = `${props.config?.plot?.project_url(fetcher.project_slug)}`;
			return await axios(get_options(url, fetcher.token))
				.then((resp) => {
					return {
						project: resp?.data,
					};
				})
				.catch((error) => {
					console.error(error);
					return EMPTY_OBJECT;
				});
		}
		const search_params = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.perf_query)) {
			if (value) {
				search_params.set(key, value);
			}
		}
		const url = `${props.config?.plot?.perf_url(
			project_slug(),
		)}?${search_params.toString()}`;
		return await axios(get_options(url, fetcher.token))
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return EMPTY_OBJECT;
			});
	};

	const [perf_data] = createResource(perf_query_fetcher, get_perf);

	// Initialize as empty, wait for resources to load
	const [reports_tab, setReportsTab] = createStore([]);
	const [branches_tab, setBranchesTab] = createStore([]);
	const [testbeds_tab, setTestbedsTab] = createStore([]);
	const [benchmarks_tab, setBenchmarksTab] = createStore([]);

	// Resource tabs data: Branches, Testbeds, Benchmarks
	const getPerfTab = async (
		perf_tab: PerfTab,
		fetcher: { per_page: number; page: number; token: string },
	) => {
		const EMPTY_ARRAY = [];
		if (props.is_console && !validate_jwt(fetcher.token)) {
			return EMPTY_ARRAY;
		}
		if (!validate_u32(fetcher.per_page.toString())) {
			return EMPTY_ARRAY;
		}
		if (!validate_u32(fetcher.page.toString())) {
			return EMPTY_ARRAY;
		}
		const search_params = new URLSearchParams();
		search_params.set("per_page", fetcher.per_page.toString());
		search_params.set("page", fetcher.page.toString());
		const url = `${props.config?.plot?.tab_url(
			project_slug(),
			perf_tab,
		)}?${search_params.toString()}`;
		return await axios(get_options(url, fetcher.token))
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				return EMPTY_ARRAY;
			});
	};

	const reports_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			per_page: reports_per_page(),
			page: reports_page(),
			refresh: refresh(),
			token: props.user?.token,
		};
	});
	const [reports_data] = createResource(reports_fetcher, async (fetcher) =>
		getPerfTab(PerfTab.REPORTS, fetcher),
	);
	createEffect(() => {
		if (
			!validate_u32(searchParams[REPORTS_PER_PAGE_PARAM]) ||
			!validate_u32(searchParams[REPORTS_PAGE_PARAM])
		) {
			setSearchParams({
				[REPORTS_PER_PAGE_PARAM]: REPORTS_PER_PAGE,
				[REPORTS_PAGE_PARAM]: DEFAULT_PAGE,
			});
		}
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
			token: props.user?.token,
		};
	});
	const [branches_data] = createResource(branches_fetcher, async (fetcher) =>
		getPerfTab(PerfTab.BRANCHES, fetcher),
	);
	createEffect(() => {
		if (
			!validate_u32(searchParams[BRANCHES_PER_PAGE_PARAM]) ||
			!validate_u32(searchParams[BRANCHES_PAGE_PARAM])
		) {
			setSearchParams({
				[BRANCHES_PER_PAGE_PARAM]: DEFAULT_PER_PAGE,
				[BRANCHES_PAGE_PARAM]: DEFAULT_PAGE,
			});
		}
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
			token: props.user?.token,
		};
	});
	const [testbeds_data] = createResource(testbeds_fetcher, async (fetcher) =>
		getPerfTab(PerfTab.TESTBEDS, fetcher),
	);
	createEffect(() => {
		if (
			!validate_u32(searchParams[TESTBEDS_PER_PAGE_PARAM]) ||
			!validate_u32(searchParams[TESTBEDS_PAGE_PARAM])
		) {
			setSearchParams({
				[TESTBEDS_PER_PAGE_PARAM]: DEFAULT_PER_PAGE,
				[TESTBEDS_PAGE_PARAM]: DEFAULT_PAGE,
			});
		}
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
			token: props.user?.token,
		};
	});
	const [benchmarks_data] = createResource(
		benchmarks_fetcher,
		async (fetcher) => getPerfTab(PerfTab.BENCHMARKS, fetcher),
	);
	createEffect(() => {
		if (
			!validate_u32(searchParams[BENCHMARKS_PER_PAGE_PARAM]) ||
			!validate_u32(searchParams[BENCHMARKS_PAGE_PARAM])
		) {
			setSearchParams({
				[BENCHMARKS_PER_PAGE_PARAM]: DEFAULT_PER_PAGE,
				[BENCHMARKS_PAGE_PARAM]: DEFAULT_PAGE,
			});
		}
		const data = benchmarks_data();
		if (data) {
			setBenchmarksTab(resourcesToCheckable(data, benchmarks()));
		}
	});

	const handleMetricKind = (metric_kind: string) => {
		setSearchParams({
			[CLEAR_PARAM]: true,
			[REPORT_PARAM]: null,
			[METRIC_KIND_PARAM]: validate_string(metric_kind, is_valid_slug)
				? metric_kind
				: null,
		});
	};

	const handleReportChecked = (index: number, metric_kind_slug: string) => {
		let report = reports_tab?.[index]?.resource;
		setSearchParams({
			[CLEAR_PARAM]: true,
			[LOWER_BOUNDARY_PARAM]: null,
			[UPPER_BOUNDARY_PARAM]: null,
			[REPORT_PARAM]: report?.uuid,
			[METRIC_KIND_PARAM]: metric_kind_slug,
			[BRANCHES_PARAM]: report?.branch?.uuid,
			[TESTBEDS_PARAM]: report?.testbed?.uuid,
			[BENCHMARKS_PARAM]: arrayToString(
				report?.results?.[0]
					?.find((result) => result.metric_kind?.slug === metric_kind_slug)
					?.benchmarks?.map((benchmark) => benchmark.uuid),
			),
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
		setSearchParams({ [START_TIME_PARAM]: date_to_time(date) });
	const handleEndTime = (date: string) =>
		setSearchParams({ [END_TIME_PARAM]: date_to_time(date) });

	const handleTab = (tab: PerfTab) => {
		if (is_perf_tab(tab)) {
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

	const handleRange = (range: Range) => {
		if (is_range(range)) {
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
				user={props.user}
				config={props.config?.header}
				path_params={props.path_params}
				perf_data={perf_data}
				is_plot_init={is_plot_init}
				perf_query={perf_query}
				handleRefresh={handleRefresh}
			/>
			<PerfPlot
				user={props.user}
				project_slug={project_slug()}
				config={props.config?.plot}
				path_params={props.path_params}
				is_console={props.is_console}
				is_plot_init={is_plot_init}
				metric_kind={metric_kind}
				report={report}
				branches={branches}
				testbeds={testbeds}
				benchmarks={benchmarks}
				start_date={start_date}
				end_date={end_date}
				refresh={refresh}
				perf_data={perf_data}
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
