import type { Params } from "astro";
import {
	type Accessor,
	Match,
	type Resource,
	Switch,
	createResource,
	Show,
} from "solid-js";
import { Button } from "../../../../config/types";
import type { JsonAuthUser } from "../../../../types/bencher";
import { BACK_PARAM, encodePath, pathname } from "../../../../util/url";
import ConsoleButton from "./ConsoleButton";
import PerfButton from "./PerfButton";
import StatusButton from "./StatusButton";
import { authUser } from "../../../../util/auth";

export interface Props {
	isConsole: boolean;
	apiUrl: string;
	params: Params;
	user: JsonAuthUser;
	button: DeckHeaderButtonConfig;
	path: Accessor<string>;
	data: Resource<object>;
	title: Accessor<string | number | undefined>;
	handleRefresh: () => void;
}

export interface DeckHeaderButtonConfig {
	kind: Button;
	path?: (pathname: string) => string;
	is_allowed?: (apiUrl: string, params: Params) => boolean;
}

const DeckHeaderButton = (props: Props) => {
	const [isAllowed] = createResource(props.params, (params) =>
		props.button.is_allowed?.(props.apiUrl, params),
	);

	return (
		<Switch>
			<Match when={props.button.kind === Button.EDIT}>
				<Show
					when={isAllowed()}
					fallback={
						<button
							type="button"
							class="button is-fullwidth"
							title={`Edit ${props.title()}`}
							disabled={true}
						>
							<span class="icon">
								<i class="fas fa-pen" />
							</span>
							<span>Edit</span>
						</button>
					}
				>
					<a
						class="button is-fullwidth"
						title={`Edit ${props.title()}`}
						href={`${
							props.button?.path?.(pathname()) ?? "#"
						}?${BACK_PARAM}=${encodePath()}`}
					>
						<span class="icon">
							<i class="fas fa-pen" />
						</span>
						<span>Edit</span>
					</a>
				</Show>
			</Match>
			<Match when={props.button.kind === Button.STATUS && isAllowed()}>
				<StatusButton
					apiUrl={props.apiUrl}
					user={props.user}
					path={props.path}
					data={props.data}
					handleRefresh={props.handleRefresh}
				/>
			</Match>
			<Match when={props.button.kind === Button.CONSOLE && props.user?.token}>
				<ConsoleButton
					params={props.params}
					resource={props.button.resource}
					param={props.button.param}
				/>
			</Match>
			<Match when={props.button.kind === Button.PERF}>
				<PerfButton
					isConsole={props.isConsole}
					params={props.params}
					data={props.data}
				/>
			</Match>
			<Match when={props.button.kind === Button.REFRESH}>
				<button
					class="button is-fullwidth"
					type="button"
					title={`Refresh ${props.title()}`}
					onMouseDown={(e) => {
						e.preventDefault();
						props.handleRefresh();
					}}
				>
					<span class="icon">
						<i class="fas fa-sync-alt" />
					</span>
					<span>Refresh</span>
				</button>
			</Match>
		</Switch>
	);
};

export default DeckHeaderButton;
