import { Switch, type Accessor, type Resource, Match } from "solid-js";
import type { JsonAuthUser } from "../../../../types/bencher";
import { ActionButton } from "../../../../config/types";
import DeleteButton from "./DeleteButton";
import type { Params } from "astro";

export interface Props {
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	config: DeckButtonConfig;
	path: Accessor<string>;
	data: Resource<object>;
}

export interface DeckButtonConfig {
	kind: ActionButton;
	subtitle: string;
	path: (pathname: string, data: object) => string;
	is_allowed?: (apiUrl: string, data: object) => boolean;
}

const DeckButton = (props: Props) => {
	return (
		<div class="columns">
			<div class="column">
				<form class="box">
					<div class="field">
						<p class="control">
							<Switch>
								<Match
									when={
										props.config?.kind === ActionButton.DELETE &&
										props.config?.is_allowed
											? props.config?.is_allowed?.(props.apiUrl, props.params)
											: true
									}
								>
									<DeleteButton
										apiUrl={props.apiUrl}
										user={props.user}
										path={props.path}
										data={props.data}
										subtitle={props.config.subtitle}
										redirect={props.config.path}
									/>
								</Match>
							</Switch>
						</p>
					</div>
				</form>
			</div>
		</div>
	);
};

export default DeckButton;
