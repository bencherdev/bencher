import { type Accessor, For, Show } from "solid-js";
import { PerfTab } from "../../../../../config/types";
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

const DimensionsTab = (props: {
	project_slug: Accessor<undefined | string>;
	isConsole: boolean;
	tab: Accessor<PerfTab>;
	tabList: Accessor<TabList<JsonBranch | JsonTestbed | JsonBenchmark>>;
	search: Accessor<undefined | string>;
	handleChecked: (index: number, slug?: string) => void;
	handleSearch: FieldHandler;
}) => {
	console.log(props.search());
	return (
		<>
			<div class="panel-block is-block">
				<Field
					kind={FieldKind.SEARCH}
					fieldKey="search"
					value={props.search() ?? ""}
					config={{
						placeholder: `Search ${props.tab()}...`,
					}}
					handleField={props.handleSearch}
				/>
			</div>
			<For each={props.tabList()}>
				{(dimension, index) => (
					<DimensionRow
						project_slug={props.project_slug}
						isConsole={props.isConsole}
						tab={props.tab}
						dimension={dimension}
						index={index}
						handleChecked={props.handleChecked}
					/>
				)}
			</For>
		</>
	);
};

const DimensionRow = (props: {
	project_slug: Accessor<undefined | string>;
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
				<a
					class="level-left"
					style="color: black;"
					title={`${props.dimension?.checked ? "Remove" : "Add"} ${
						resource?.name
					}`}
					// biome-ignore lint/a11y/useValidAnchor: stateful anchor
					onClick={(_e) => props.handleChecked(props.index())}
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
				<Show when={props.isConsole}>
					<div class="level-right">
						<div class="level-item">
							<ViewDimensionButton
								project_slug={props.project_slug}
								tab={props.tab}
								dimension={resource}
							/>
						</div>
					</div>
				</Show>
			</div>
		</div>
	);
};

const ViewDimensionButton = (props: {
	project_slug: Accessor<undefined | string>;
	tab: Accessor<PerfTab>;
	dimension: JsonBranch | JsonTestbed | JsonBenchmark | JsonMeasure;
}) => {
	return (
		<a
			class="button"
			title={`View ${props.dimension?.name}`}
			href={`/console/projects/${props.project_slug()}/${props.tab()}/${
				props.dimension?.slug
			}?${BACK_PARAM}=${encodePath()}`}
		>
			View
		</a>
	);
};

export default DimensionsTab;
