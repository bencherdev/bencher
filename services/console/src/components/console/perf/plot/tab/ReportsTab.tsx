import {
	type Accessor,
	For,
	Show,
	Switch,
	Match,
	createSignal,
} from "solid-js";
import type { PerfTab } from "../../../../../config/types";
import { fmtDateTime, resourcePath } from "../../../../../config/util";
import type { JsonReport } from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import { BRANCH_ICON } from "../../../../../config/project/branches";
import { TESTBED_ICON } from "../../../../../config/project/testbeds";
import { BENCHMARK_ICON } from "../../../../../config/project/benchmarks";
import { MEASURE_ICON } from "../../../../../config/project/measures";
import { ALERT_ICON } from "../../../../../config/project/alerts";
import type { TabElement, TabList } from "./PlotTab";
import DateRange from "../../../../field/kinds/DateRange";
import { themeText, type Theme } from "../../../../navbar/theme/theme";
import ReportCard from "../../../deck/hand/card/ReportCard";

const ReportsTab = (props: {
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	loading: Accessor<boolean>;
	report: Accessor<undefined | string>;
	measures: Accessor<string[]>;
	tab: Accessor<PerfTab>;
	tabList: Accessor<TabList<JsonReport>>;
	per_page: Accessor<number>;
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
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
								<div class="level">
									{/* biome-ignore lint/a11y/useValidAnchor: loading fallback */}
									<a class={`level-left ${themeText(props.theme())}`}>
										<div class="level-item">
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
										</div>
									</a>
									<div class="level-right">
										<div class="level-item">
											{/* biome-ignore lint/a11y/useValidAnchor: loading fallback */}
											<a class="button">
												{props.isConsole ? "Manage" : "View"}
											</a>
										</div>
									</div>
								</div>
							</div>
						)}
					</For>
				</Match>
				<Match when={props.tabList().length > 0}>
					<For each={props.tabList()}>
						{(report, index) => (
							<ReportRow
								project_slug={props.project_slug}
								theme={props.theme}
								isConsole={props.isConsole}
								measures={props.measures}
								tab={props.tab}
								report={report}
								isActive={props.report() === report.resource.uuid}
								index={index}
								handleChecked={props.handleChecked}
							/>
						)}
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
	report: TabElement<JsonReport>;
	isActive: boolean;
	index: Accessor<number>;
	handleChecked: (index: number, slug?: string) => void;
}) => {
	const report = props.report?.resource as JsonReport;
	const benchmarkCount = report?.results?.map((iteration) => iteration?.length);
	const hasBenchmarks = benchmarkCount.length > 0;

	const [viewReport, setViewReport] = createSignal(
		props.isActive && hasBenchmarks,
	);

	return (
		<div id={report.uuid} class="panel-block is-block">
			<div class="level">
				{/* biome-ignore lint/a11y/useValidAnchor: action on press */}
				<a
					class={`level-left ${themeText(props.theme())}`}
					title={`View Report from ${fmtDateTime(report?.start_time)}`}
					disabled={!hasBenchmarks}
					onMouseDown={(e) => {
						e.preventDefault();
						setViewReport(!viewReport());
						props.handleChecked(props.index(), report?.uuid);
					}}
				>
					<div class="level-item">
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
								<small style="word-break: break-word;">
									{fmtDateTime(report?.start_time)}
								</small>
								<ReportDimension
									icon={BRANCH_ICON}
									name={report?.branch?.name}
								/>
								<ReportDimension
									icon={TESTBED_ICON}
									name={report?.testbed?.name}
								/>
								<ReportDimension
									icon={BENCHMARK_ICON}
									name={(() => {
										if (benchmarkCount.length === 0) {
											return "0 benchmarks";
										}
										const plural =
											benchmarkCount.length > 1 ||
											benchmarkCount.filter((count) => count > 1).length > 0;
										return `${benchmarkCount.join(" x ")} benchmark${
											plural ? "s" : ""
										}`;
									})()}
								/>
								<ReportDimension
									icon={MEASURE_ICON}
									name={(() => {
										const counts = report?.results?.map((iteration) =>
											iteration?.reduce((acc, result) => {
												const c = result?.measures?.length ?? 0;
												if (!acc.has(c)) {
													acc.add(c);
												}
												return acc;
											}, new Set<number>()),
										);
										if (counts.length === 0) {
											return "0 measures";
										}
										const plural =
											counts.length > 1 ||
											counts.some(
												(count) =>
													count.size > 1 ||
													Array.from(count).some((c) => c > 1),
											);
										return `${counts
											.map((iteration) => Array.from(iteration).join(" + "))
											.join(" x ")} measure${plural ? "s" : ""}`;
									})()}
								/>
								<Show when={report?.alerts?.length > 0}>
									<ReportDimension
										icon={ALERT_ICON}
										iconClass=" has-text-primary"
										name={(() => {
											const count = report?.alerts.length;
											return `${count} alert${count > 0 ? "s" : ""}`;
										})()}
									/>
								</Show>
							</div>
						</div>
					</div>
				</a>
				<div class="level-right">
					<div class="level-item">
						<ViewReportButton
							project_slug={props.project_slug}
							isConsole={props.isConsole}
							tab={props.tab}
							report={report}
						/>
					</div>
				</div>
			</div>
			<Show when={viewReport()}>
				<ReportCard
					isConsole={props.isConsole}
					params={{
						project: props.project_slug(),
					}}
					value={() => report}
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
			class="button"
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
			<span class="icon-text ">
				<span class={`icon${props.iconClass ?? ""}`}>
					<i class={props.icon} />
				</span>
				<small style="word-break: break-all;">{props.name}</small>
			</span>
		</div>
	);
};

export default ReportsTab;
