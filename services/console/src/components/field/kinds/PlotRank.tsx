import { createMemo } from "solid-js";
import type { FieldValueHandler } from "../Field";

export type PlotRankValue = number | null | undefined;

export interface Props {
	value: PlotRankValue;
	valid: undefined | null | boolean;
	config: PlotRankConfig;
	handleField: FieldValueHandler;
}

export interface PlotRankConfig {
	bottom: string;
	top: string;
	total?: number;
}

enum Button {
	Bottom = 256,
	Top = 1,
	Custom = 0,
}

const PlotRank = (props: Props) => {
	const isSelected = (button: Button) => {
		const IS_SELECTED = " is-primary is-selected";
		switch (button) {
			case Button.Bottom:
				return (props.config?.total ?? button) === props.value
					? IS_SELECTED
					: "";
			case Button.Top:
				return button === props.value ? IS_SELECTED : "";
			case Button.Custom:
				if (props.config?.total && props.config?.total === props.value) {
					return "";
				}
				return props.value !== Button.Bottom && props.value !== Button.Top
					? IS_SELECTED
					: "";
		}
	};

	const rank = createMemo(() => {
		switch (props.value) {
			case Button.Bottom:
			case Button.Top:
				return null;
			default:
				if (props.config?.total && props.config?.total === props.value) {
					return null;
				}
				return props.value;
		}
	});

	return (
		<>
			<div class="buttons has-addons">
				<button
					type="button"
					class={`button is-small${isSelected(Button.Bottom)}`}
					title={props.config?.bottom}
					onClick={(e) => {
						e.preventDefault();
						props.handleField(props.config?.total ?? Button.Bottom);
					}}
				>
					<span class="icon is-small">
						<i class="fas fa-angle-double-down" />
					</span>
					<span>{props.config?.bottom}</span>
				</button>
				<button
					type="button"
					class={`button is-small${isSelected(Button.Top)}`}
					title={props.config?.top}
					onClick={(e) => {
						e.preventDefault();
						props.handleField(Button.Top);
					}}
				>
					<span class="icon is-small">
						<i class="fas fa-angle-double-up" />
					</span>
					<span>{props.config?.top}</span>
				</button>
			</div>
			<div class="field has-addons">
				<p class="control">
					<button
						type="button"
						class={`button is-small${isSelected(Button.Custom)}`}
						title="Custom location"
					>
						Custom
					</button>
				</p>
				<p class="control">
					<input
						class="input is-small"
						type="number"
						placeholder={props.value?.toString()}
						value={rank()}
						onInput={(event) => props.handleField(event.target?.value)}
					/>
				</p>
			</div>
		</>
	);
};

export default PlotRank;
