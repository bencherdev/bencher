import axios from "axios";
import { useSearchParams } from "solid-app-router";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { isPerfTab, is_range, PerfTab, Range } from "../../config/types";
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

const METRIC_KIND_PARAM = "metric_kind";
const BRANCHES_PARAM = "branches";
const TESTBEDS_PARAM = "testbeds";
const BENCHMARKS_PARAM = "benchmarks";
const START_TIME_PARAM = "start_time";
const END_TIME_PARAM = "end_time";

const BRANCHES_PAGE_PARAM = "branches_page";
const TESTBEDS_PAGE_PARAM = "testbeds_page";
const BENCHMARKS_PAGE_PARAM = "benchmarks_page";

const TAB_PARAM = "tab";
const KEY_PARAM = "key";
const RANGE_PARAM = "range";

const DEFAULT_PERF_TAB = PerfTab.BRANCHES;
const DEFAULT_PERF_KEY = true;
const DEFAULT_PERF_RANGE = Range.DATE_TIME;

const DEFAULT_PER_PAGE = 8;
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

const time_to_date = (time_str: undefined | string): Date => {
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

const time_to_date_iso = (time_str: undefined | string): string => {
	const date = time_to_date(time_str);
	if (date) {
		return date.toISOString();
	}
	return null;
};

const time_to_date_only_iso = (time_str: undefined | string): string => {
	const iso_date = time_to_date_iso(time_str);
	if (iso_date) {
		return iso_date.split("T")?.[0];
	}
	return null;
};

const date_to_time = (date_str: undefined | string): string => {
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
			uuid: resource?.uuid,
			name: resource?.name,
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
	if (!isPerfTab(searchParams[TAB_PARAM])) {
		setSearchParams({ [TAB_PARAM]: null });
	}
	if (!is_bool_param(searchParams[KEY_PARAM])) {
		setSearchParams({ [KEY_PARAM]: DEFAULT_PERF_KEY });
	}
	if (!is_range(searchParams[RANGE_PARAM])) {
		setSearchParams({ [RANGE_PARAM]: null });
	}

	// Sanitize all pagination query params
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
		if (isPerfTab(searchParams[TAB_PARAM])) {
			return searchParams[TAB_PARAM];
		} else {
			return DEFAULT_PERF_TAB;
		}
	});

	const key = createMemo(() => {
		// This check is required for the initial load
		// before the query params have been sanitized
		if (is_bool_param(searchParams[KEY_PARAM])) {
			return searchParams[KEY_PARAM] === "true";
		} else {
			return DEFAULT_PERF_KEY;
		}
	});

	const range = createMemo(() => {
		// This check is required for the initial load
		// before the query params have been sanitized
		if (is_range(searchParams[RANGE_PARAM])) {
			return searchParams[RANGE_PARAM];
		} else {
			return DEFAULT_PERF_RANGE;
		}
	});

	// Pagination query params
	const per_page = createMemo(() => DEFAULT_PER_PAGE);
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

	const project_slug = createMemo(() => props.path_params()?.project_slug);
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
		if (!validate_jwt(fetcher.token)) {
			return EMPTY_OBJECT;
		}
		// Don't even send query if there isn't at least one: branch, testbed, and benchmark
		if (isPlotInit()) {
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
	const [branches_tab, setBranchesTab] = createStore([]);
	const [testbeds_tab, setTestbedsTab] = createStore([]);
	const [benchmarks_tab, setBenchmarksTab] = createStore([]);

	// Resource tabs data: Branches, Testbeds, Benchmarks
	const getPerfTab = async (
		perf_tab: PerfTab,
		fetcher: { per_page: number; page: number; token: string },
	) => {
		const EMPTY_ARRAY = [];
		if (!validate_jwt(fetcher.token)) {
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

	const branches_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			per_page: per_page(),
			page: branches_page(),
			refresh: refresh(),
			token: props.user?.token,
		};
	});
	const [branches_data] = createResource(branches_fetcher, async (fetcher) =>
		getPerfTab(PerfTab.BRANCHES, fetcher),
	);
	createEffect(() => {
		if (!validate_u32(searchParams[BRANCHES_PAGE_PARAM])) {
			setSearchParams({ [BRANCHES_PAGE_PARAM]: DEFAULT_PAGE });
		}
		const data = branches_data();
		if (data) {
			setBranchesTab(resourcesToCheckable(data, branches()));
		}
	});

	const testbeds_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			per_page: per_page(),
			page: testbeds_page(),
			refresh: refresh(),
			token: props.user?.token,
		};
	});
	const [testbeds_data] = createResource(testbeds_fetcher, async (fetcher) =>
		getPerfTab(PerfTab.TESTBEDS, fetcher),
	);
	createEffect(() => {
		if (!validate_u32(searchParams[TESTBEDS_PAGE_PARAM])) {
			setSearchParams({ [TESTBEDS_PAGE_PARAM]: DEFAULT_PAGE });
		}
		const data = testbeds_data();
		if (data) {
			setTestbedsTab(resourcesToCheckable(data, testbeds()));
		}
	});

	const benchmarks_fetcher = createMemo(() => {
		return {
			project_slug: project_slug(),
			per_page: per_page(),
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
		if (!validate_u32(searchParams[BENCHMARKS_PAGE_PARAM])) {
			setSearchParams({ [BENCHMARKS_PAGE_PARAM]: DEFAULT_PAGE });
		}
		const data = benchmarks_data();
		if (data) {
			setBenchmarksTab(resourcesToCheckable(data, benchmarks()));
		}
	});

	const handleMetricKind = (metric_kind: string) => {
		setSearchParams({
			[METRIC_KIND_PARAM]: validate_string(metric_kind, is_valid_slug)
				? metric_kind
				: null,
		});
	};

	const handleChecked = (
		resource_tab: any[],
		index: number,
		param: string,
		param_array: string[],
		uuid: string,
	) => {
		const checked = resource_tab?.[index].checked;
		if (typeof checked !== "boolean") {
			return;
		}
		if (checked) {
			setSearchParams({
				[param]: arrayToString(removeFromArray(param_array, uuid)),
			});
		} else {
			setSearchParams({
				[param]: arrayToString(addToArray(param_array, uuid)),
			});
		}
	};
	const handleBranchChecked = (index: number, uuid: string) => {
		handleChecked(branches_tab, index, BRANCHES_PARAM, branches(), uuid);
	};
	const handleTestbedChecked = (index: number, uuid: string) => {
		handleChecked(testbeds_tab, index, TESTBEDS_PARAM, testbeds(), uuid);
	};
	const handleBenchmarkChecked = (index: number, uuid: string) => {
		handleChecked(benchmarks_tab, index, BENCHMARKS_PARAM, benchmarks(), uuid);
	};

	const handleStartTime = (date: string) =>
		setSearchParams({ [START_TIME_PARAM]: date_to_time(date) });
	const handleEndTime = (date: string) =>
		setSearchParams({ [END_TIME_PARAM]: date_to_time(date) });

	const handleTab = (tab: PerfTab) => {
		if (isPerfTab(tab)) {
			setSearchParams({ [TAB_PARAM]: tab });
		}
	};

	const handleKey = (key: boolean) => {
		if (typeof key === "boolean") {
			setSearchParams({ [KEY_PARAM]: key });
		}
	};

	const handleRange = (range: Range) => {
		if (is_range(range)) {
			setSearchParams({ [RANGE_PARAM]: range });
		}
	};

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
				isPlotInit={isPlotInit}
				perf_query={perf_query}
				handleRefresh={handleRefresh}
			/>
			<PerfPlot
				user={props.user}
				project_slug={project_slug()}
				config={props.config?.plot}
				path_params={props.path_params}
				is_console={props.is_console}
				isPlotInit={isPlotInit}
				metric_kind={metric_kind}
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
				branches_tab={branches_tab}
				testbeds_tab={testbeds_tab}
				benchmarks_tab={benchmarks_tab}
				per_page={per_page}
				branches_page={branches_page}
				testbeds_page={testbeds_page}
				benchmarks_page={benchmarks_page}
				handleMetricKind={handleMetricKind}
				handleStartTime={handleStartTime}
				handleEndTime={handleEndTime}
				handleTab={handleTab}
				handleKey={handleKey}
				handleRange={handleRange}
				handleBranchChecked={handleBranchChecked}
				handleTestbedChecked={handleTestbedChecked}
				handleBenchmarkChecked={handleBenchmarkChecked}
				handleBranchesPage={handleBranchesPage}
				handleTestbedsPage={handleTestbedsPage}
				handleBenchmarksPage={handleBenchmarksPage}
			/>
		</>
	);
};

export default PerfPanel;
