import { Accessor, For, Resource, createEffect, createMemo } from "solid-js";
import type { JsonAuthUser } from "../../../../types/bencher";
import { Params, pathname, useNavigate } from "../../../../util/url";
import { fmtValues, setPageTitle } from "../../../../util/resource";
import DeckHeaderButton, { DeckHeaderButtonConfig } from "./DeckHeaderButton";

export interface Props {
	pathParams: Params;
	user: JsonAuthUser;
	config: DeckHeaderConfig;
	url: Accessor<string>;
	data: Resource<Record<string, any>>;
	handleRefresh: () => void;
}

export interface DeckHeaderConfig {
	key: string;
	keys?: string[][];
	path: (pathname: string) => string;
	path_to: string;
	buttons: DeckHeaderButtonConfig[];
}

const DeckHeader = (props: Props) => {
	const navigate = useNavigate();

	const title = createMemo(() =>
		fmtValues(props.data(), props.config?.key, props.config?.keys, " | "),
	);

	createEffect(() => {
		setPageTitle(title()?.toString());
	});

	return (
		<div class="columns is-centered">
			<div class="column is-narrow">
				<button
					class="button is-outlined is-fullwidth"
					title={`Back to ${props.config?.path_to}`}
					onClick={(e) => {
						e.preventDefault();
						navigate(props.config?.path(pathname()));
					}}
				>
					<span class="icon">
						<i class="fas fa-chevron-left" aria-hidden="true" />
					</span>
					<span>Back</span>
				</button>
			</div>
			<div class="column">
				<div class="content has-text-centered">
					<h3 class="title is-3" style="overflow-wrap:anywhere;">
						{title()}
					</h3>
				</div>
			</div>

			<div class="column is-narrow">
				<nav class="level">
					<div class="level-right">
						<For each={props.config?.buttons}>
							{(button) => (
								<div class="level-item">
									<DeckHeaderButton
										pathParams={props.pathParams}
										user={props.user}
										button={button}
										url={props.url}
										data={props.data}
										title={title}
										handleRefresh={props.handleRefresh}
									/>
								</div>
							)}
						</For>
					</div>
				</nav>
			</div>
		</div>
	);
};

export default DeckHeader;
