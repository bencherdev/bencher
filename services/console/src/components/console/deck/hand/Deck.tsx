import { createElementSize } from "@solid-primitives/resize-observer";
import type { Params } from "astro";
import {
	type Accessor,
	For,
	type Resource,
	Show,
	createEffect,
	createSignal,
} from "solid-js";
import type { JsonAuthUser } from "../../../../types/bencher";
import DeckButton, { type DeckButtonConfig } from "./DeckButton";
import type CardConfig from "./card/CardConfig";
import DeckCard from "./card/DeckCard";

export interface Props {
	isConsole?: boolean;
	apiUrl: string;
	isBencherCloud: boolean;
	params: Params;
	user: JsonAuthUser;
	config: DeckConfig;
	path: Accessor<string>;
	data: Resource<object>;
	handleRefresh: () => void;
	handleLoopback: (pathname: null | string) => void;
}

export interface DeckConfig {
	url: (params: Params, search?: Params) => string;
	top_buttons?: DeckButtonConfig[];
	cards: CardConfig[];
	buttons?: DeckButtonConfig[];
}

const Deck = (props: Props) => {
	let deckRef: HTMLDivElement | undefined;
	const deckSize = createElementSize(() => deckRef);
	const [width, setWidth] = createSignal(null);
	createEffect(() => {
		if (!width()) {
			setWidth(deckSize.width);
		}
	});

	return (
		<div
			ref={(e) => {
				deckRef = e;
			}}
		>
			<Show when={props.config?.top_buttons}>
				<For each={props.config?.top_buttons}>
					{(button) => (
						<DeckButton
							apiUrl={props.apiUrl}
							params={props.params}
							user={props.user}
							config={button}
							path={props.path}
							data={props.data}
							handleRefresh={props.handleRefresh}
						/>
					)}
				</For>
			</Show>
			<For each={props.config?.cards}>
				{(card) => (
					<DeckCard
						isConsole={props.isConsole === true}
						apiUrl={props.apiUrl}
						isBencherCloud={props.isBencherCloud}
						params={props.params}
						user={props.user}
						path={props.path}
						card={card}
						data={props.data}
						width={width}
						handleRefresh={props.handleRefresh}
						handleLoopback={props.handleLoopback}
					/>
				)}
			</For>
			<br />
			<Show when={props.config?.buttons}>
				<For each={props.config?.buttons}>
					{(button) => (
						<DeckButton
							apiUrl={props.apiUrl}
							params={props.params}
							user={props.user}
							config={button}
							path={props.path}
							data={props.data}
							handleRefresh={props.handleRefresh}
						/>
					)}
				</For>
			</Show>
		</div>
	);
};

export default Deck;
