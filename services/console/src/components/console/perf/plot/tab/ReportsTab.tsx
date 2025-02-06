import { type Accessor, For, Match, Show, Switch, createMemo } from "solid-js";
import {
	ALERT_ICON,
	ALERT_OFF_ICON,
} from "../../../../../config/project/alerts";
import { BENCHMARK_ICON } from "../../../../../config/project/benchmarks";
import { BRANCH_ICON } from "../../../../../config/project/branches";
import { MEASURE_ICON } from "../../../../../config/project/measures";
import { TESTBED_ICON } from "../../../../../config/project/testbeds";
import type { PerfTab } from "../../../../../config/types";
import { fmtDateTime, resourcePath } from "../../../../../config/util";
import { AlertStatus, type JsonReport } from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import DateRange from "../../../../field/kinds/DateRange";
import { type Theme, themeText } from "../../../../navbar/theme/theme";
import ReportCard, {
	boundaryLimitsMap,
} from "../../../deck/hand/card/ReportCard";
import type { TabElement, TabList } from "./PlotTab";

const ADAPTER_ICON = "fas fa-plug";

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
									{/* biome-ignore lint/a11y/useValidAnchor: loading fallback */}
									<a class={`column ${themeText(props.theme())}`}>
										<div class="columns is-vcentered is-mobile">
											<div class="column is-narrow">
												<span class="icon is-small">
													<i class="fas fa-plus" />
												</span>
											</div>
											<div class="column">
												<small style="word-break: break-word;">⠀</small>
												<ReportDimension icon={BRANCH_ICON} name="⠀" />
												<ReportDimension icon={TESTBED_ICON} name="⠀" />
												<ReportDimension icon={BENCHMARK_ICON} name="⠀" />
												<ReportDimension icon={MEASURE_ICON} name="⠀" />
											</div>
										</div>
									</a>
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
				{/* biome-ignore lint/a11y/useValidAnchor: action on press */}
				<a
					class={`column ${themeText(props.theme())}`}
					title={`View Report from ${fmtDateTime(report?.start_time)}`}
					disabled={!hasBenchmarks}
					onMouseDown={(e) => {
						e.preventDefault();
						props.handleChecked(props.index(), report.uuid);
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
							<ReportRowFields report={report} />
						</div>
					</div>
				</a>
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

export const ReportRowFields = (props: { report: JsonReport }) => {
	const benchmarkCount = props.report?.results?.map(
		(iteration) => iteration?.length,
	);

	const totalAlerts = props.report?.alerts?.length;
	const activeAlerts = props.report?.alerts?.filter(
		(alert) => alert.status === AlertStatus.Active,
	).length;

	return (
		<div>
			<small style="word-break: break-word;">
				{fmtDateTime(props.report?.start_time)}
			</small>
			<Show when={totalAlerts}>
				<ReportDimension
					icon={activeAlerts === 0 ? ALERT_OFF_ICON : ALERT_ICON}
					iconClass={activeAlerts === 0 ? "" : " has-text-primary"}
					name={(() => {
						const active =
							activeAlerts === 0 || activeAlerts === totalAlerts
								? ""
								: ` (${activeAlerts} active | ${totalAlerts - activeAlerts} inactive)`;
						return `${totalAlerts} ${totalAlerts === 1 ? "alert" : "alerts"}${active}`;
					})()}
				/>
			</Show>
			<ReportDimension icon={BRANCH_ICON} name={props.report?.branch?.name} />
			<ReportDimension icon={TESTBED_ICON} name={props.report?.testbed?.name} />
			<ReportDimension
				icon={BENCHMARK_ICON}
				name={(() => {
					if (benchmarkCount.length === 0) {
						return "0 benchmarks";
					}
					const plural =
						benchmarkCount.length > 1 ||
						benchmarkCount.some((count) => count > 1);
					return `${benchmarkCount.join(" + ")} benchmark${plural ? "s" : ""}`;
				})()}
			/>
			<ReportDimension
				icon={MEASURE_ICON}
				name={(() => {
					const measureCount = props.report?.results?.map(
						(iteration) => boundaryLimitsMap(iteration).size,
					);
					if (measureCount.length === 0) {
						return "0 measures";
					}
					const plural =
						measureCount.length > 1 || measureCount.some((count) => count > 1);
					return `${measureCount.join(" + ")} measure${plural ? "s" : ""}`;
				})()}
			/>
			<ReportDimension
				icon={ADAPTER_ICON}
				name={props.report?.adapter ?? "Mystery"}
			/>
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

const ReportDimension = (props: {
	icon: string;
	name: string;
	iconClass?: string;
}) => {
	return (
		<div>
			<span class="icon-text">
				<span class={`icon${props.iconClass ?? ""}`}>
					<i class={props.icon} />
				</span>
				<small style="word-break: break-word;">{props.name}</small>
			</span>
		</div>
	);
};

export default ReportsTab;
