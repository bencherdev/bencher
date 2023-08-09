import { Accessor, For, Resource, Show } from "solid-js";
import type { Params } from "../../../../util/url";
import type { JsonAuthUser } from "../../../../types/bencher";
import DeckButton, { DeckButtonConfig } from "./DeckButton";
import DeckCard from "./card/DeckCard";
import type CardConfig from "./card/CardConfig";

export interface Props {
	pathParams: Params;
	user: JsonAuthUser;
	config: DeckConfig;
	url: Accessor<string>;
	data: Resource<Record<string, any>>;
	handleRefresh: () => void;
	handleLoopback: (pathname: null | string) => void;
}

export interface DeckConfig {
	cards: CardConfig[];
	buttons: DeckButtonConfig[];
}

const Deck = (props: Props) => {
	return (
		<>
			<For each={props.config?.cards}>
				{(card) => (
					<div class="columns">
						<div class="column">
							<div class="card">
								<DeckCard
									pathParams={props.pathParams}
									user={props.user}
									url={props.url}
									card={card}
									data={props.data}
									// refresh={props.refresh}
									handleRefresh={props.handleRefresh}
									handleLoopback={props.handleLoopback}
								/>
							</div>
						</div>
					</div>
				)}
			</For>
			<Show when={props.config?.buttons} fallback={<></>}>
				<For each={props.config?.buttons}>
					{(button) => (
						<DeckButton
							user={props.user}
							config={button}
							url={props.url}
							data={props.data}
						/>
					)}
				</For>
			</Show>
		</>
	);
};

export default Deck;
