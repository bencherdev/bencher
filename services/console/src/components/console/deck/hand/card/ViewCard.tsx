import { Switch, Match, createResource } from "solid-js";
import { Display } from "../../../../../config/types";
import type CardConfig from "./CardConfig";
import type { Params } from "astro";

export interface Props {
	apiUrl: string;
	params: Params;
	card: CardConfig;
	value: boolean | string;
	toggleUpdate: () => void;
}

const ViewCard = (props: Props) => {
	const [is_allowed] = createResource(props.params, (params) =>
		props.card?.is_allowed?.(props.apiUrl, params),
	);

	return (
		<div class="card">
			<div class="card-header">
				<div class="card-header-title">{props.card?.label}</div>
			</div>
			<div class="card-content">
				<div class="content">
					<Switch fallback={<></>}>
						<Match when={props.card?.display === Display.RAW}>
							<p style="overflow-wrap:anywhere;">{props.value}</p>
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
								} else {
									return field;
								}
							}, props.value)}
						</Match>
					</Switch>
				</div>
			</div>
			{is_allowed() && (
				<div class="card-footer">
					<a
						class="card-footer-item"
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
