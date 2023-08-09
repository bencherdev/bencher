import { Switch, Match, createMemo, createResource } from "solid-js";
import type { Params } from "../../../../../util/url";
import { Display } from "../../../../../config/types";
import type CardConfig from "./CardConfig";

export interface Props {
	pathParams: Params;
	card: CardConfig;
	value: boolean | string;
	toggleUpdate: () => void;
}

const ViewCard = (props: Props) => {
	const [is_allowed] = createResource(props.pathParams, (pathParams) =>
		props.card?.is_allowed?.(pathParams),
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
