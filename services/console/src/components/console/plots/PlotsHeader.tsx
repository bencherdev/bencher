import { Show, type Accessor, type Resource } from "solid-js";
import type { Params } from "astro";
import Field from "../../field/Field";
import FieldKind from "../../field/kind";
import type { JsonProject } from "../../../types/bencher";

export interface Props {
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
					{/* biome-ignore lint/complexity/noUselessFragments: non-breaking space */}
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
					<Show
						when={props.isAllowedCreate.loading || props.isAllowedCreate()}
						fallback={
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
					</Show>
				</p>
				<p class="level-item">
					<button
						class="button"
						type="button"
						title="Refresh all plots"
						onClick={(e) => {
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
