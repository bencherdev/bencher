import { Accessor, Match, Resource, Switch } from "solid-js";
import type { JsonAuthUser } from "../../../../types/bencher";
import { Params, pathname, useNavigate } from "../../../../util/url";
import { Button } from "../../../../config/types";
import StatusButton from "./StatusButton";
import PerfButton from "./PerfButton";

export interface Props {
	pathParams: Params;
	user: JsonAuthUser;
	button: DeckHeaderButtonConfig;
	url: Accessor<string>;
	data: Resource<Record<string, any>>;
	title: Accessor<string | number | undefined>;
	handleRefresh: () => void;
}

export interface DeckHeaderButtonConfig {
	kind: Button;
	path?: (pathname: string) => string;
}

const DeckHeaderButton = (props: Props) => {
	const navigate = useNavigate();

	return (
		<Switch fallback={<></>}>
			<Match when={props.button.kind === Button.EDIT}>
				<button
					class="button is-outlined is-fullwidth"
					title={`Edit ${props.title()}`}
					onClick={(e) => {
						e.preventDefault();
						const path = props.button?.path?.(pathname());
						if (path) {
							navigate(path);
						}
					}}
				>
					<span class="icon">
						<i class="fas fa-pen" aria-hidden="true" />
					</span>
					<span>Edit</span>
				</button>
			</Match>
			<Match when={props.button.kind === Button.STATUS}>
				<StatusButton
					user={props.user}
					url={props.url}
					data={props.data}
					handleRefresh={props.handleRefresh}
				/>
			</Match>
			<Match when={props.button.kind === Button.PERF}>
				<PerfButton pathParams={props.pathParams} data={props.data} />
			</Match>
			<Match when={props.button.kind === Button.REFRESH}>
				<button
					class="button is-outlined is-fullwidth"
					title={`Refresh ${props.title()}`}
					onClick={(e) => {
						e.preventDefault();
						props.handleRefresh();
					}}
				>
					<span class="icon">
						<i class="fas fa-sync-alt" aria-hidden="true" />
					</span>
					<span>Refresh</span>
				</button>
			</Match>
		</Switch>
	);
};

export default DeckHeaderButton;
