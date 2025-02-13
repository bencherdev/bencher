import type { Params } from "astro";
import { type Accessor, Show, createSignal } from "solid-js";
import type { JsonAuthUser } from "../../../../../types/bencher";
import type CardConfig from "./CardConfig";
import UpdateCard from "./UpdateCard";
import ViewCard from "./ViewCard";

export interface Props {
	isConsole?: boolean;
	apiUrl: string;
	isBencherCloud: boolean;
	params: Params;
	user: JsonAuthUser;
	path: Accessor<string>;
	card: CardConfig;
	value: boolean | string;
	handleRefresh: () => void;
	handleLoopback: (pathname: null | string) => void;
}

const FieldCard = (props: Props) => {
	const [update, setUpdate] = createSignal(false);

	const toggleUpdate = () => {
		setUpdate(!update());
	};

	return (
		<Show
			when={update()}
			fallback={
				<ViewCard
					isConsole={props.isConsole}
					apiUrl={props.apiUrl}
					isBencherCloud={props.isBencherCloud}
					params={props.params}
					card={props.card}
					value={props.value}
					toggleUpdate={toggleUpdate}
				/>
			}
		>
			<UpdateCard
				apiUrl={props.apiUrl}
				params={props.params}
				user={props.user}
				path={props.path}
				card={props.card}
				value={props.value}
				toggleUpdate={toggleUpdate}
				handleRefresh={props.handleRefresh}
				handleLoopback={props.handleLoopback}
			/>
		</Show>
	);
};

export default FieldCard;
