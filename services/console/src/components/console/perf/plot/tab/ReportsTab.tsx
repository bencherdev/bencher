import * as Sentry from "@sentry/astro";
import {
	type Accessor,
	For,
	Match,
	Show,
	Switch,
	createMemo,
	createResource,
} from "solid-js";
import { BENCHMARK_ICON } from "../../../../../config/project/benchmarks";
import { BRANCH_ICON } from "../../../../../config/project/branches";
import { MEASURE_ICON } from "../../../../../config/project/measures";
import { TESTBED_ICON } from "../../../../../config/project/testbeds";
import type { PerfTab } from "../../../../../config/types";
import { fmtDateTime, resourcePath } from "../../../../../config/util";
import type { JsonReport } from "../../../../../types/bencher";
import { authUser } from "../../../../../util/auth";
import { httpGet } from "../../../../../util/http";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import DateRange from "../../../../field/kinds/DateRange";
import type { Theme } from "../../../../navbar/theme/theme";
import ReportCard, {} from "../../../deck/hand/card/ReportCard";
import type { TabElement, TabList } from "./PlotTab";
import TableReportRow from "../../../table/rows/ReportRow";
import DimensionLabel from "../../../table/rows/DimensionLabel";

const ReportsTab = (props: {
	apiUrl: string;
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	loading: Accessor<boolean>;
	report: Accessor<undefined | string>;
	measures: Accessor<string[]>;
	tab: Accessor<PerfTab>;
	tabList: Accessor<TabList<JsonReport>>;
	page: Accessor<number>;
	per_page: Accessor<number>;
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	width: Accessor<number>;
	handleChecked: (index: number, slug?: string) => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
}) => {
	return (
		<>
			<div class="panel-block is-block">
				<DateRange
					start_date={props.start_date}
					end_date={props.end_date}
					handleStartTime={props.handleStartTime}
					handleEndTime={props.handleEndTime}
				/>
			</div>
			<Switch fallback={<div class="panel-block">🐰 No reports found</div>}>
				<Match when={props.loading()}>
					<For each={Array(props.per_page())}>
						{(_) => (
							<div class="panel-block is-block">
								<div class="columns is-vcentered">
									<div class="column">
										<div class="columns is-vcentered is-mobile">
											<div class="column is-narrow">
												<span class="icon is-small">
													<i class="fas fa-plus" />
												</span>
											</div>
											<div class="column">
												<small style="word-break: break-word;">⠀</small>
												<DimensionLabel icon={BRANCH_ICON} name="⠀" />
												<DimensionLabel icon={TESTBED_ICON} name="⠀" />
												<DimensionLabel icon={BENCHMARK_ICON} name="⠀" />
												<DimensionLabel icon={MEASURE_ICON} name="⠀" />
											</div>
										</div>
									</div>
									<div class="column is-narrow">
										{/* biome-ignore lint/a11y/useValidAnchor: loading fallback */}
										<a class="button is-fullwidth">
											{props.isConsole ? "Manage" : "View"}
										</a>
									</div>
								</div>
							</div>
						)}
					</For>
				</Match>
				<Match when={props.tabList().length > 0}>
					<For each={props.tabList()}>
						{(report, index) => {
							return (
								<ReportRow
									apiUrl={props.apiUrl}
									project_slug={props.project_slug}
									theme={props.theme}
									isConsole={props.isConsole}
									measures={props.measures}
									tab={props.tab}
									width={props.width}
									report={report}
									index={index}
									isChecked={report?.resource?.uuid === props.report()}
									handleChecked={props.handleChecked}
								/>
							);
						}}
					</For>
				</Match>
			</Switch>
		</>
	);
};

const ReportRow = (props: {
	apiUrl: string;
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	measures: Accessor<string[]>;
	tab: Accessor<PerfTab>;
	width: Accessor<number>;
	report: TabElement<JsonReport>;
	index: Accessor<number>;
	isChecked: boolean;
	handleChecked: (index: number, uuid: string) => void;
}) => {
	const report = props.report?.resource as JsonReport;
	const hasBenchmarks =
		(report?.counts?.results?.reduce(
			(acc, counts) => acc + (counts?.benchmarks ?? 0),
			0,
		) ?? 0) > 0;

	const viewReport = createMemo(() => props.isChecked && hasBenchmarks);

	// The reports list endpoint does not include the results,
	// so fetch the full report when it is expanded.
	const [fullReport] = createResource(
		() => (viewReport() ? report?.uuid : undefined),
		async (uuid) => {
			const path = `/v0/projects/${props.project_slug()}/reports/${uuid}`;
			return await httpGet(props.apiUrl, path, authUser()?.token)
				.then((resp) => resp?.data)
				.catch((error) => {
					console.error(error);
					Sentry.captureException(error);
					return undefined;
				});
		},
	);

	return (
		<div id={report.uuid} class="panel-block is-block">
			<div class="columns is-vcentered">
				<div
					class="column"
					title={`View Report from ${fmtDateTime(report?.start_time)}`}
					style={hasBenchmarks ? { cursor: "pointer" } : {}}
					onMouseDown={(e) => {
						e.preventDefault();
						if (hasBenchmarks) {
							props.handleChecked(props.index(), report.uuid);
						}
					}}
				>
					<div class="columns is-vcentered is-mobile">
						<div class="column is-narrow">
							<Show
								when={viewReport()}
								fallback={
									<span
										class={`icon is-small${
											hasBenchmarks ? "" : " has-text-grey"
										}`}
									>
										<i class="fas fa-plus" />
									</span>
								}
							>
								<span class="icon is-small">
									<i class="fas fa-minus" />
								</span>
							</Show>
						</div>
						<div class="column">
							<TableReportRow report={report} />
						</div>
					</div>
				</div>
				<div class="column is-narrow">
					<ViewReportButton
						project_slug={props.project_slug}
						isConsole={props.isConsole}
						tab={props.tab}
						report={report}
					/>
				</div>
			</div>
			<Show when={viewReport()}>
				<Show
					when={fullReport()}
					fallback={
						<progress
							class="progress is-primary"
							style="margin-top: 1rem;"
							max="100"
						/>
					}
				>
					<ReportCard
						isConsole={props.isConsole}
						params={{
							project: props.project_slug(),
						}}
						value={fullReport}
						width={props.width}
					/>
				</Show>
			</Show>
		</div>
	);
};

const ViewReportButton = (props: {
	project_slug: Accessor<undefined | string>;
	isConsole: boolean;
	tab: Accessor<PerfTab>;
	report: JsonReport;
}) => {
	return (
		<a
			class="button is-fullwidth"
			title={`${props.isConsole ? "Manage" : "View"} Report from ${fmtDateTime(
				props.report?.start_time,
			)}`}
			href={`${resourcePath(
				props.isConsole,
			)}/${props.project_slug()}/${props.tab()}/${
				props.report?.uuid
			}?${BACK_PARAM}=${encodePath()}`}
		>
			{props.isConsole ? "Manage" : "View"}
		</a>
	);
};

export default ReportsTab;
