import { type Accessor, For, Show, Switch, Match, createMemo } from "solid-js";
import type { PerfTab } from "../../../../../config/types";
import { fmtDateTime } from "../../../../../config/util";
import type { JsonPlot, JsonReport } from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import type { TabElement, TabList } from "./PlotTab";
import { themeText, type Theme } from "../../../../navbar/theme/theme";
import Field, { type FieldHandler } from "../../../../field/Field";
import FieldKind from "../../../../field/kind";

const PlotsTab = (props: {
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	loading: Accessor<boolean>;
	measures: Accessor<string[]>;
	tab: Accessor<PerfTab>;
	tabList: Accessor<TabList<JsonReport>>;
	per_page: Accessor<number>;
	search: Accessor<undefined | string>;
	handleChecked: (index: number, slug?: string) => void;
	handleSearch: FieldHandler;
}) => {
	return (
		<>
			<div class="panel-block is-block">
				<Field
					kind={FieldKind.SEARCH}
					fieldKey="search"
					value={props.search() ?? ""}
					config={{
						placeholder: "Search Pinned Plots",
					}}
					handleField={props.handleSearch}
				/>
			</div>
			<Switch fallback={<div class="panel-block">🐰 No plots found</div>}>
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
												</div>
											</div>
										</div>
									</a>
								</div>
							</div>
						)}
					</For>
				</Match>
				<Match when={props.tabList().length > 0}>
					<For each={props.tabList()}>
						{(plot, index) => (
							<PlotRow
								project_slug={props.project_slug}
								theme={props.theme}
								isConsole={props.isConsole}
								measures={props.measures}
								tab={props.tab}
								plot={plot}
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

const PlotRow = (props: {
	theme: Accessor<Theme>;
	isConsole: boolean;
	plot: TabElement<JsonPlot>;
	index: Accessor<number>;
	tab: Accessor<PerfTab>;
	handleChecked: (index: number, slug?: string) => void;
}) => {
	const plot = createMemo(() => props.plot.resource);

	return (
		<div class="panel-block is-block">
			<div class="level">
				<a
					class={`level-left ${themeText(props.theme())}`}
					onClick={(_e) => props.handleChecked(props.index?.(), plot().uuid)}
				>
					<div class="level-item">
						<div class="columns is-vcentered is-mobile">
							<div class="column is-narrow">
								<input type="radio" checked={props.plot?.checked} />
							</div>
							<div class="column">
								<small style="word-break: break-word;">
									{plot().title ??
										`Unnamed Plot (${fmtDateTime(plot().created)})`}
								</small>
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
								plot={plot}
							/>
						</div>
					</div>
				</Show>
			</div>
		</div>
	);
};

const ViewReportButton = (props: {
	project_slug: Accessor<undefined | string>;
	tab: Accessor<PerfTab>;
	plot: Accessor<JsonPlot>;
}) => {
	return (
		<a
			class="button"
			title="Manage Pinned Plot"
			href={`/console/projects/${props.project_slug()}/${props.tab()}/${
				props.plot()?.uuid
			}`}
		>
			Manage
		</a>
	);
};

export default PlotsTab;
