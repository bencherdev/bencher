import { For } from "solid-js";

import DeckCard from "./DeckCard";
import DeckButton from "./DeckButton";

const Deck = (props) => {
	return (
		<>
			{props.config?.buttons && (
				<DeckButton
					config={props.config?.buttons}
					path_params={props.path_params}
				/>
			)}
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
			{props.config?.buttons && (
				<DeckButton
					config={props.config?.buttons}
					path_params={props.path_params}
				/>
			)}
		</>
	);
};

export default Deck;
