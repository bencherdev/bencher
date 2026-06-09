import type { Params } from "astro";
import { Show, createSignal } from "solid-js";
import type { BencherResource } from "../../../config/types";
import type {
	JsonProjectKeyCreated,
	JsonUserKeyCreated,
} from "../../../types/bencher";
import KeyCreated from "./KeyCreated";
import PosterPanel from "./PosterPanel";

export type JsonKeyCreated = JsonProjectKeyCreated | JsonUserKeyCreated;

interface Props {
	apiUrl: string;
	params: Params;
	resource: BencherResource;
}

const KeyAddPanel = (props: Props) => {
	const [created, setCreated] = createSignal<JsonKeyCreated | null>(null);

	return (
		<Show
			when={created()}
			fallback={
				<PosterPanel
					apiUrl={props.apiUrl}
					params={props.params}
					resource={props.resource}
					onSuccess={(data) => setCreated(data as unknown as JsonKeyCreated)}
				/>
			}
		>
			{(data) => (
				<KeyCreated
					params={props.params}
					resource={props.resource}
					data={data()}
				/>
			)}
		</Show>
	);
};

export default KeyAddPanel;
