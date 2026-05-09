import type { Params } from "astro";
import { Show, createSignal } from "solid-js";
import { BencherResource } from "../../../config/types";
import type { JsonProjectKeyCreated } from "../../../types/bencher";
import KeyCreated from "./KeyCreated";
import PosterPanel from "./PosterPanel";

interface Props {
	apiUrl: string;
	params: Params;
}

const KeyAddPanel = (props: Props) => {
	const [created, setCreated] = createSignal<JsonProjectKeyCreated | null>(
		null,
	);

	return (
		<Show
			when={created()}
			fallback={
				<PosterPanel
					apiUrl={props.apiUrl}
					params={props.params}
					resource={BencherResource.PROJECT_KEYS}
					onSuccess={(data) =>
						setCreated(data as unknown as JsonProjectKeyCreated)
					}
				/>
			}
		>
			{(data) => <KeyCreated params={props.params} data={data()} />}
		</Show>
	);
};

export default KeyAddPanel;
