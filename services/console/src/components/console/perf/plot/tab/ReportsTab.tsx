import { type Accessor, For, Match, Show, Switch, createMemo } from "solid-js";
import { BENCHMARK_ICON } from "../../../../../config/project/benchmarks";
import { BRANCH_ICON } from "../../../../../config/project/branches";
import { MEASURE_ICON } from "../../../../../config/project/measures";
import { TESTBED_ICON } from "../../../../../config/project/testbeds";
import type { PerfTab } from "../../../../../config/types";
import { fmtDateTime, resourcePath } from "../../../../../config/util";
import type { JsonReport } from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import DateRange from "../../../../field/kinds/DateRange";
import type { Theme } from "../../../../navbar/theme/theme";
import ReportCard, {} from "../../../deck/hand/card/ReportCard";
import type { TabElement, TabList } from "./PlotTab";
import TableReportRow from "../../../table/rows/ReportRow";
import DimensionLabel from "../../../table/rows/DimensionLabel";

const ReportsTab = (props: {
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
		report?.results
			?.map((iteration) => iteration?.length)
			?.reduce((acc, n) => acc + n, 0) > 0;

	const viewReport = createMemo(() => props.isChecked && hasBenchmarks);

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
				<ReportCard
					isConsole={props.isConsole}
					params={{
						project: props.project_slug(),
					}}
					value={() => report}
					width={props.width}
				/>
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
