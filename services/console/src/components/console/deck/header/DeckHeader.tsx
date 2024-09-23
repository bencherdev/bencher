import type { Params } from "astro";
import {
	type Accessor,
	For,
	type Resource,
	createEffect,
	createMemo,
} from "solid-js";
import type { JsonAuthUser } from "../../../../types/bencher";
import { fmtValues, setPageTitle } from "../../../../util/resource";
import { decodePath, pathname } from "../../../../util/url";
import DeckHeaderButton, {
	type DeckHeaderButtonConfig,
} from "./DeckHeaderButton";
import { Display } from "../../../../config/types";
import { fmtDateTime } from "../../../../config/util";

export interface Props {
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	config: DeckHeaderConfig;
	path: Accessor<string>;
	data: Resource<object>;
	handleRefresh: () => void;
}

export interface DeckHeaderConfig {
	key: string;
	keys?: string[][];
	display?: Display;
	path: (pathname: string) => string;
	path_to: string;
	buttons: DeckHeaderButtonConfig[];
}

const DeckHeader = (props: Props) => {
	const title = createMemo(() => {
		const data = props.data();
		if (props.data.loading || !data) {
			return;
		}
		switch (props.config?.display) {
			case Display.DATE_TIME:
				return fmtDateTime(data?.[props.config?.key] ?? "");
			default:
				return fmtValues(data, props.config?.key, props.config?.keys, " | ");
		}
	});

	createEffect(() => {
		setPageTitle(title()?.toString());
	});

	return (
		<div class="columns is-centered">
			<div class="column is-narrow">
				<a
					class="button is-fullwidth"
					title={`Back to ${props.config?.path_to}`}
					href={decodePath(props.config?.path(pathname()))}
				>
					<span class="icon">
						<i class="fas fa-chevron-left" />
					</span>
					<span>Back</span>
				</a>
			</div>
			<div class="column">
				<div class="content has-text-centered">
					<h3 class="title is-3" style="word-break: break-word;">
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
										isConsole={true}
										apiUrl={props.apiUrl}
										params={props.params}
										user={props.user}
										button={button}
										path={props.path}
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
