import { type Accessor, For, Show, Switch, Match } from "solid-js";
import type { PerfTab } from "../../../../../config/types";
import type {
	JsonBenchmark,
	JsonBranch,
	JsonMeasure,
	JsonTestbed,
} from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import type { TabElement, TabList } from "./PlotTab";
import Field, { type FieldHandler } from "../../../../field/Field";
import FieldKind from "../../../../field/kind";
import { themeText, type Theme } from "../../../../navbar/theme/theme";
import { toCapitalized } from "../../../../../config/util";

const DimensionsTab = (props: {
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	loading: Accessor<boolean>;
	tab: Accessor<PerfTab>;
	tabUuids: Accessor<string[]>;
	tabList: Accessor<TabList<JsonBranch | JsonTestbed | JsonBenchmark>>;
	per_page: Accessor<number>;
	search: Accessor<undefined | string>;
	handleChecked: (index?: number, slug?: string) => void;
	handleSearch: FieldHandler;
}) => {
	return (
		<>
			<div class="panel-block is-block">
				<div class="columns is-vcentered">
					<div class="column">
						<Field
							kind={FieldKind.SEARCH}
							fieldKey="search"
							value={props.search() ?? ""}
							config={{
								placeholder: `Search ${toCapitalized(props.tab())}`,
							}}
							handleField={props.handleSearch}
						/>
					</div>
					<Show when={props.tabUuids().length > 0}>
						<div class="column is-narrow">
							<button
								type="button"
								class="button is-small is-fullwidth"
								onMouseDown={(e) => {
									e.preventDefault();
									props.handleChecked();
								}}
							>
								Clear {toCapitalized(props.tab())}
							</button>
						</div>
					</Show>
				</div>
			</div>
			<Switch
				fallback={<div class="panel-block">🐰 No {props.tab()} found</div>}
			>
				<Match when={props.loading()}>
					<For each={Array(props.per_page())}>
						{(_) => (
							<div class="panel-block is-block">
								<div class="level">
									<div class={`level-left ${themeText(props.theme())}`}>
										<div class="level-item">
											<div class="columns is-vcentered is-mobile">
												<div class="column is-narrow">
													<input type="checkbox" checked={false} />
												</div>
												<div class="column">
													<small style="word-break: break-word;" />
												</div>
											</div>
										</div>
									</div>
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
						{(dimension, index) => (
							<DimensionRow
								project_slug={props.project_slug}
								theme={props.theme}
								isConsole={props.isConsole}
								tab={props.tab}
								dimension={dimension}
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

const DimensionRow = (props: {
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	tab: Accessor<PerfTab>;
	dimension: TabElement<JsonBranch | JsonTestbed | JsonBenchmark>;
	index: Accessor<number>;
	handleChecked: (index: number, slug?: string) => void;
}) => {
	const resource = props.dimension?.resource as
		| JsonBranch
		| JsonTestbed
		| JsonBenchmark
		| JsonMeasure;
	return (
		<div class="panel-block is-block">
			<div class="level">
				{/* biome-ignore lint/a11y/useValidAnchor: action on press */}
				<a
					class={`level-left ${themeText(props.theme())}`}
					title={`${props.dimension?.checked ? "Remove" : "Add"} ${
						resource?.name
					}`}
					onMouseDown={(_e) => props.handleChecked(props.index())}
				>
					<div class="level-item">
						<div class="columns is-vcentered is-mobile">
							<div class="column is-narrow">
								<input type="checkbox" checked={props.dimension?.checked} />
							</div>
							<div class="column">
								<small style="word-break: break-word;">{resource?.name}</small>
							</div>
						</div>
					</div>
				</a>
				<div class="level-right">
					<div class="level-item">
						<ViewDimensionButton
							project_slug={props.project_slug}
							isConsole={props.isConsole}
							tab={props.tab}
							dimension={resource}
						/>
					</div>
				</div>
			</div>
		</div>
	);
};

const ViewDimensionButton = (props: {
	project_slug: Accessor<undefined | string>;
	isConsole: boolean;
	tab: Accessor<PerfTab>;
	dimension: JsonBranch | JsonTestbed | JsonBenchmark | JsonMeasure;
}) => {
	return (
		<a
			class="button"
			title={`${props.isConsole ? "Manage" : "View"} ${props.dimension?.name}`}
			href={`${props.isConsole ? "/console/projects" : "/perf"}/
			${props.project_slug()}/${props.tab()}/${
				props.dimension?.slug
			}?${BACK_PARAM}=${encodePath()}`}
		>
			{props.isConsole ? "Manage" : "View"}
		</a>
	);
};

export default DimensionsTab;
