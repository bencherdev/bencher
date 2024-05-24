import type { Params } from "astro";
import { Match, Show, Switch, createMemo, createResource } from "solid-js";
import { Display } from "../../../../../config/types";
import type CardConfig from "./CardConfig";

export interface Props {
	apiUrl: string;
	params: Params;
	card: CardConfig;
	value: boolean | string | object;
	toggleUpdate: () => void;
}

const ViewCard = (props: Props) => {
	const [is_allowed] = createResource(props.params, (params) =>
		props.card?.is_allowed?.(props.apiUrl, params),
	);

	return (
		<div id={props.card?.label} class="card">
			<div class="card-header">
				<div class="card-header-title">{props.card?.label}</div>
			</div>
			<div class="card-content">
				<div class="content">
					<Switch>
						<Match when={props.card?.display === Display.RAW}>
							<p style="word-break: break-word;">{props.value}</p>
						</Match>
						<Match when={props.card?.display === Display.SWITCH}>
							<div class="field">
								<input
									type="checkbox"
									class="switch"
									checked={
										typeof props.value === "boolean" ? props.value : false
									}
									disabled={true}
								/>
								<label />
							</div>
						</Match>
						<Match when={props.card?.display === Display.SELECT}>
							{props.card?.field?.value?.options.reduce((field, option) => {
								if (props.value === option.value) {
									return option.option;
								}
								return field;
							}, props.value)}
						</Match>
						<Match when={props.card?.display === Display.START_POINT}>
							<Show when={props.value}>
								<a
									href={`/console/projects/${props.params?.project}/branches/${props.value?.branch}`}
								>
									View Start Point
									<br />
									Version Number: {props.value?.version?.number}
									<br />
									{props.value?.version?.hash && (
										<>Version Hash: {props.value?.version?.hash}</>
									)}
								</a>
							</Show>
						</Match>
					</Switch>
				</div>
			</div>
			{is_allowed() && (
				<div class="card-footer">
					<a
						class="card-footer-item"
						// biome-ignore lint/a11y/useValidAnchor: card link
						onClick={(e) => {
							e.preventDefault();
							props.toggleUpdate();
						}}
					>
						Update
					</a>
				</div>
			)}
		</div>
	);
};

export default ViewCard;
