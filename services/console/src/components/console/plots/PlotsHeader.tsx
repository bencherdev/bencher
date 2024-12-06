import type { Params } from "astro";
import { type Accessor, Match, type Resource, Switch } from "solid-js";
import { perfPath } from "../../../config/util";
import type { JsonProject } from "../../../types/bencher";
import Field from "../../field/Field";
import FieldKind from "../../field/kind";

export interface Props {
	isConsole: boolean;
	apiUrl: string;
	params: Params;
	project: Resource<JsonProject>;
	search: Accessor<undefined | string>;
	isAllowedCreate: Resource<boolean>;
	handleRefresh: () => void;
	handleSearch: (search: string) => void;
}

const PlotsHeader = (props: Props) => {
	return (
		<nav class="level">
			<div class="level-left">
				<div class="level-item">
					<h3 class="title is-3">{props.project()?.name ?? <>&nbsp;</>}</h3>
				</div>
			</div>

			<div class="level-right">
				<p class="level-item">
					<Field
						kind={FieldKind.SEARCH}
						fieldKey="search"
						value={props.search() ?? ""}
						config={{
							placeholder: "Search Plots",
						}}
						handleField={(_key, search, _valid) =>
							props.handleSearch(search as string)
						}
					/>
				</p>
				<p class="level-item">
					<Switch>
						<Match
							when={
								props.isConsole &&
								(props.isAllowedCreate.loading || props.isAllowedCreate())
							}
						>
							<a
								class="button"
								title="Add a plot"
								href={`/console/projects/${
									props.project()?.slug
								}/perf?clear=true`}
							>
								<span class="icon">
									<i class="fas fa-plus" />
								</span>
								<span>Add</span>
							</a>
						</Match>
						<Match when={props.isConsole && !props.isAllowedCreate()}>
							<button
								type="button"
								class="button"
								title="Add a plot"
								disabled={true}
							>
								<span class="icon">
									<i class="fas fa-plus" />
								</span>
								<span>Add</span>
							</button>
						</Match>
					</Switch>
				</p>
				<div class="level-item">
					<a
						class="button is-fullwidth"
						title={`View ${props.project()?.name} Perf`}
						href={perfPath(props.isConsole, props.project()?.slug)}
					>
						<span class="icon">
							<i class="fas fa-chart-line" />
						</span>
						<span>Perf</span>
					</a>
				</div>
				<p class="level-item">
					<button
						class="button"
						type="button"
						title="Refresh all plots"
						onMouseDown={(e) => {
							e.preventDefault();
							props.handleRefresh();
						}}
					>
						<span class="icon">
							<i class="fas fa-sync-alt" />
						</span>
						<span>Refresh</span>
					</button>
				</p>
			</div>
		</nav>
	);
};

export default PlotsHeader;
