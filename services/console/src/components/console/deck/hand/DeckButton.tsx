import { Switch, type Accessor, type Resource, Match } from "solid-js";
import type { JsonAuthUser } from "../../../../types/bencher";
import { ActionButton } from "../../../../config/types";
import DeleteButton from "./DeleteButton";
import type { Params } from "astro";

export interface Props {
	params: Params;
	user: JsonAuthUser;
	config: DeckButtonConfig;
	url: Accessor<string>;
	data: Resource<Record<string, any>>;
}

export interface DeckButtonConfig {
	kind: ActionButton;
	subtitle: string;
	path: (pathname: string, data: Record<string, any>) => string;
	is_allowed?: (data: Record<string, any>) => boolean;
}

const DeckButton = (props: Props) => {
	return (
		<div class="columns">
			<div class="column">
				<form class="box">
					<div class="field">
						<p class="control">
							<Switch fallback={<></>}>
								<Match
									when={
										props.config?.kind === ActionButton.DELETE &&
										props.config?.is_allowed
											? props.config?.is_allowed?.(props.params)
											: true
									}
								>
									<DeleteButton
										user={props.user}
										url={props.url}
										data={props.data}
										subtitle={props.config.subtitle}
										path={props.config.path}
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
