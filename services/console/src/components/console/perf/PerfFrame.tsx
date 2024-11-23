import * as Sentry from "@sentry/astro";
import { debounce } from "@solid-primitives/scheduled";
import type { Params } from "astro";
import {
	type Resource,
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
	project_slug: () => string;
	refresh: () => number;
	handleRefresh: () => void;
	reports_data: Resource<JsonReport>;
	branches_data: Resource<JsonBranch>;
	testbeds_data: Resource<JsonTestbed>;
	benchmarks_data: Resource<JsonBenchmark>;
	plots_data: Resource<JsonPlot>;
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
	tab: () => PerfTab;
	key: () => boolean;
	x_axis: () => XAxis;
	clear: () => boolean;
	lower_value: () => boolean;
	upper_value: () => boolean;
	lower_boundary: () => boolean;
	upper_boundary: () => boolean;
	reports_per_page: () => number;
	branches_per_page: () => number;
	testbeds_per_page: () => number;
	benchmarks_per_page: () => number;
	plots_per_page: () => number;
	reports_page: () => number;
	branches_page: () => number;
	testbeds_page: () => number;
	benchmarks_page: () => number;
	plots_page: () => number;
	reportsTotalCount: () => number;
	branchesTotalCount: () => number;
	testbedsTotalCount: () => number;
	benchmarksTotalCount: () => number;
	plotsTotalCount: () => number;
	reports_start_time: () => string;
	reports_end_time: () => string;
	reports_start_date: () => string;
	reports_end_date: () => string;
	branches_search: () => string;
	testbeds_search: () => string;
	benchmarks_search: () => string;
	plots_search: () => string;
	reports_tab: TabList<JsonReport>;
	branches_tab: TabList<JsonBranch>;
	testbeds_tab: TabList<JsonTestbed>;
	benchmarks_tab: TabList<JsonBenchmark>;
	plots_tab: TabList<JsonPlot>;
	handleReportChecked: (index: number) => void;
	handleBranchChecked: (index: number) => void;
	handleTestbedChecked: (index: number) => void;
	handleBenchmarkChecked: (index: number) => void;
	handleMeasure: (measure: null | string) => void;
	handlePlotChecked: (index: number) => void;
	handleStartTime: (date: string) => void;
	handleEndTime: (date: string) => void;
	handleTab: (tab: PerfTab) => void;
	handleKey: (key: boolean) => void;
	handleXAxis: (x_axis: XAxis) => void;
	handleClear: (clear: boolean) => void;
	handleLowerValue: (end: boolean) => void;
	handleUpperValue: (end: boolean) => void;
	handleLowerBoundary: (boundary: boolean) => void;
	handleUpperBoundary: (boundary: boolean) => void;
	handleReportsPage: (page: number) => void;
	handleBranchesPage: (page: number) => void;
	handleTestbedsPage: (page: number) => void;
	handleBenchmarksPage: (page: number) => void;
	handlePlotsPage: (page: number) => void;
	handleReportsStartTime: (date: string) => void;
	handleReportsEndTime: (date: string) => void;
	handleBranchesSearch: (search: string) => void;
	handleTestbedsSearch: (search: string) => void;
	handleBenchmarksSearch: (search: string) => void;
	handlePlotsSearch: (search: string) => void;
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

	const projectFetcher = createMemo(() => {
		return {
			project_slug: props.project_slug(),
			refresh: props.refresh(),
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
			refresh: props.refresh(),
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
					handleRefresh={props.handleRefresh}
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
				refresh={props.refresh}
				perfData={perfData}
				tab={props.tab}
				key={props.key}
				x_axis={props.x_axis}
				clear={props.clear}
				lower_value={props.lower_value}
				upper_value={props.upper_value}
				lower_boundary={props.lower_boundary}
				upper_boundary={props.upper_boundary}
				reports_data={props.reports_data}
				branches_data={props.branches_data}
				testbeds_data={props.testbeds_data}
				benchmarks_data={props.benchmarks_data}
				plots_data={props.plots_data}
				reports_tab={props.reports_tab}
				branches_tab={props.branches_tab}
				testbeds_tab={props.testbeds_tab}
				benchmarks_tab={props.benchmarks_tab}
				plots_tab={props.plots_tab}
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
				reports_total_count={props.reportsTotalCount}
				branches_total_count={props.branchesTotalCount}
				testbeds_total_count={props.testbedsTotalCount}
				benchmarks_total_count={props.benchmarksTotalCount}
				plots_total_count={props.plotsTotalCount}
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
				handleMeasure={props.handleMeasure}
				handleStartTime={props.handleStartTime}
				handleEndTime={props.handleEndTime}
				handleTab={props.handleTab}
				handleKey={props.handleKey}
				handleXAxis={props.handleXAxis}
				handleClear={props.handleClear}
				handleLowerValue={props.handleLowerValue}
				handleUpperValue={props.handleUpperValue}
				handleLowerBoundary={props.handleLowerBoundary}
				handleUpperBoundary={props.handleUpperBoundary}
				handleReportChecked={props.handleReportChecked}
				handleBranchChecked={props.handleBranchChecked}
				handleTestbedChecked={props.handleTestbedChecked}
				handleBenchmarkChecked={props.handleBenchmarkChecked}
				handlePlotChecked={props.handlePlotChecked}
				handleReportsPage={props.handleReportsPage}
				handleBranchesPage={props.handleBranchesPage}
				handleTestbedsPage={props.handleTestbedsPage}
				handleBenchmarksPage={props.handleBenchmarksPage}
				handlePlotsPage={props.handlePlotsPage}
				handleReportsStartTime={props.handleReportsStartTime}
				handleReportsEndTime={props.handleReportsEndTime}
				handleBranchesSearch={props.handleBranchesSearch}
				handleTestbedsSearch={props.handleTestbedsSearch}
				handleBenchmarksSearch={props.handleBenchmarksSearch}
				handlePlotsSearch={props.handlePlotsSearch}
			/>
		</>
	);
};

export default PerfFrame;
