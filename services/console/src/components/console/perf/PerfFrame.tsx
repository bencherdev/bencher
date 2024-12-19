import * as Sentry from "@sentry/astro";
import bencher_valid_init from "bencher_valid";
import {
	type Accessor,
	type Resource,
	createMemo,
	createResource,
} from "solid-js";
import type { PerfTab } from "../../../config/types";
import type {
	JsonAuthUser,
	JsonPerf,
	JsonPerfQuery,
	JsonProject,
	XAxis,
} from "../../../types/bencher";
import { httpGet } from "../../../util/http";
import {
	MAX_NOTIFY_TIMEOUT,
	NOTIFY_TIMEOUT_PARAM,
	NotifyKind,
	pageNotify,
} from "../../../util/notify";
import { validJwt } from "../../../util/valid";
import type { Theme } from "../../navbar/theme/theme";
import PerfPlot from "./plot/PerfPlot";

export interface Props {
	children?: Element | undefined;
	apiUrl: string;
	user: JsonAuthUser;
	isConsole?: boolean;
	isEmbed?: boolean;
	plotId?: string;
	theme: Accessor<Theme>;
	project?: Resource<JsonProject>;
	project_slug: Accessor<string | undefined>;
	measuresIsEmpty: Accessor<boolean>;
	branchesIsEmpty: Accessor<boolean>;
	testbedsIsEmpty: Accessor<boolean>;
	benchmarksIsEmpty: Accessor<boolean>;
	isPlotInit: Accessor<boolean>;
	perfQuery: Accessor<JsonPerfQuery>;
	refresh: Accessor<number>;
	measures: Accessor<string[]>;
	start_date: Accessor<string | undefined>;
	end_date: Accessor<string | undefined>;
	key: Accessor<boolean>;
	x_axis: Accessor<XAxis>;
	clear: Accessor<boolean>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	embed_logo: Accessor<boolean>;
	embed_title: Accessor<string | undefined>;
	embed_header: Accessor<boolean>;
	embed_key: Accessor<boolean>;
	handleMeasure: (measure: null | string) => void;
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
}

const PerfFrame = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);

	const perfFetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			project_slug: props.project_slug(),
			perfQuery: props.perfQuery(),
			refresh: props.refresh(),
			token: props.user?.token,
		};
	});
	const getPerf = async (fetcher: {
		project_slug: string;
		perfQuery: JsonPerfQuery;
		refresh: number;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		if (!bencher_valid()) {
			return EMPTY_OBJECT;
		}

		// Don't even send query if there isn't at least one: branch, testbed, and benchmark
		if (
			(props.isConsole && !validJwt(fetcher.token)) ||
			props.isPlotInit() ||
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
			.then((resp) => {
				return resp?.data;
			})
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
		<PerfPlot
			apiUrl={props.apiUrl}
			user={props.user}
			project={props.project}
			project_slug={props.project_slug}
			theme={props.theme}
			isConsole={props.isConsole === true}
			isEmbed={props.isEmbed === true}
			plotId={props.plotId}
			measuresIsEmpty={props.measuresIsEmpty}
			branchesIsEmpty={props.branchesIsEmpty}
			testbedsIsEmpty={props.testbedsIsEmpty}
			benchmarksIsEmpty={props.benchmarksIsEmpty}
			isPlotInit={props.isPlotInit}
			refresh={props.refresh}
			perfData={perfData}
			measures={props.measures}
			start_date={props.start_date}
			end_date={props.end_date}
			key={props.key}
			x_axis={props.x_axis}
			clear={props.clear}
			lower_value={props.lower_value}
			upper_value={props.upper_value}
			lower_boundary={props.lower_boundary}
			upper_boundary={props.upper_boundary}
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
		>
			{props.children}
		</PerfPlot>
	);
};

export default PerfFrame;
