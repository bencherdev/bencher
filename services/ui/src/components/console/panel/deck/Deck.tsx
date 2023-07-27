import { For, Show } from "solid-js";

import DeckCard from "./DeckCard";
import DeckButton from "./DeckButton";

const Deck = (props) => {
	return (
		<>
			<For each={props.config?.cards}>
				{(card) => (
					<div class="columns">
						<div class="column">
							<div class="card">
								<DeckCard
									user={props.user}
									card={card}
									data={props.data}
									path_params={props.path_params}
									url={props.url}
									refresh={props.refresh}
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
