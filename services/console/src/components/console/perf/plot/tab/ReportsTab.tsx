import { type Accessor, For, Show, Switch, Match } from "solid-js";
import type { PerfTab } from "../../../../../config/types";
import { fmtDateTime } from "../../../../../config/util";
import type { JsonReport } from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import { BRANCH_ICON } from "../../../../../config/project/branches";
import { TESTBED_ICON } from "../../../../../config/project/testbeds";
import { MEASURE_ICON } from "../../../../../config/project/measures";
import type { TabElement, TabList } from "./PlotTab";
import DateRange from "../../../../field/kinds/DateRange";
import { themeText, type Theme } from "../../../../navbar/theme/theme";

const ReportsTab = (props: {
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	loading: Accessor<boolean>;
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
													<input type="radio" checked={false} />
												</div>
												<div class="column">
													<small style="word-break: break-word;">⠀</small>
													<ReportDimension icon={BRANCH_ICON} name="⠀" />
													<ReportDimension icon={TESTBED_ICON} name="⠀" />
													<ReportDimension icon={MEASURE_ICON} name="⠀" />
												</div>
											</div>
										</div>
									</a>
									<Show when={props.isConsole}>
										<div class="level-right">
											<div class="level-item">
												{/* biome-ignore lint/a11y/useValidAnchor: loading fallback */}
												<a class="button">View</a>
											</div>
										</div>
									</Show>
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
	index: Accessor<number>;
	handleChecked: (index: number, slug?: string) => void;
}) => {
	const resource = props.report?.resource as JsonReport;
	return (
		<Show
			when={(resource?.results?.[0]?.length ?? 0) > 0}
			fallback={
				<div class="panel-block is-block">
					<div class="level">
						<div class={`level-left ${themeText(props.theme())}`}>
							<div class="level-item">
								<div class="columns is-vcentered is-mobile">
									<div class="column is-narrow">
										<input type="radio" disabled={true} checked={false} />
									</div>
									<div class="column">
										<small style="word-break: break-word;">
											{fmtDateTime(resource?.start_time)}
										</small>
										<ReportDimension
											icon="fab fa-creative-commons-zero"
											name="No Results"
										/>
									</div>
								</div>
							</div>
						</div>
						<Show when={props.isConsole}>
							<div class="level-right">
								<div class="level-item">
									<ViewReportButton
										project_slug={props.project_slug}
										tab={props.tab}
										report={resource}
									/>
								</div>
							</div>
						</Show>
					</div>
				</div>
			}
		>
			<For each={resource?.results?.[0]}>
				{(result, _index) => (
					<div class="panel-block is-block">
						<div class="level">
							<a
								class={`level-left ${themeText(props.theme())}`}
								title={`View Report from ${fmtDateTime(resource?.start_time)}`}
								// biome-ignore lint/a11y/useValidAnchor: stateful anchor
								onClick={(_e) =>
									// Send the Measure UUID instead of the Report UUID
									props.handleChecked(props.index?.(), result.measure?.uuid)
								}
							>
								<div class="level-item">
									<div class="columns is-vcentered is-mobile">
										<div class="column is-narrow">
											<input
												type="radio"
												checked={
													props.report?.checked &&
													result.measure?.uuid === props.measures()?.[0]
												}
											/>
										</div>
										<div class="column">
											<small style="word-break: break-word;">
												{fmtDateTime(resource?.start_time)}
											</small>
											<ReportDimension
												icon={BRANCH_ICON}
												name={resource?.branch?.name}
											/>
											<ReportDimension
												icon={TESTBED_ICON}
												name={resource?.testbed?.name}
											/>
											<ReportDimension
												icon={MEASURE_ICON}
												name={result.measure?.name}
											/>
										</div>
									</div>
								</div>
							</a>
							<Show when={props.isConsole}>
								<div class="level-right">
									<div class="level-item">
										<ViewReportButton
											project_slug={props.project_slug}
											tab={props.tab}
											report={resource}
										/>
									</div>
								</div>
							</Show>
						</div>
					</div>
				)}
			</For>
		</Show>
	);
};

const ViewReportButton = (props: {
	project_slug: Accessor<undefined | string>;
	tab: Accessor<PerfTab>;
	report: JsonReport;
}) => {
	return (
		<a
			class="button"
			title={`Manage Report from ${fmtDateTime(props.report?.start_time)}`}
			href={`/console/projects/${props.project_slug()}/${props.tab()}/${
				props.report?.uuid
			}?${BACK_PARAM}=${encodePath()}`}
		>
			Manage
		</a>
	);
};

const ReportDimension = (props: { icon: string; name: string }) => {
	return (
		<div>
			<span class="icon">
				<i class={props.icon} />
			</span>
			<small style="word-break: break-all;">{props.name}</small>
		</div>
	);
};

export default ReportsTab;
