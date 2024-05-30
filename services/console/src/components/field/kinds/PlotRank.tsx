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
	placeholder: string;
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
			case Button.Top:
				return button === props.value ? IS_SELECTED : "";
			case Button.Custom:
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
				return props.value;
		}
	});

	return (
		<>
			<div class="buttons has-addons">
				<button
					type="button"
					class={`button is-small${isSelected(Button.Bottom)}`}
					title="Insert at bottom"
					onClick={(e) => {
						e.preventDefault();
						props.handleField(Button.Bottom);
					}}
				>
					<span class="icon is-small">
						<i class="fas fa-angle-double-down" />
					</span>
					<span>Insert at bottom</span>
				</button>
				<button
					type="button"
					class={`button is-small${isSelected(Button.Top)}`}
					title="Insert at top"
					onClick={(e) => {
						e.preventDefault();
						props.handleField(Button.Top);
					}}
				>
					<span class="icon is-small">
						<i class="fas fa-angle-double-up" />
					</span>
					<span>Insert at top</span>
				</button>
			</div>
			<div class="field has-addons">
				<p class="control">
					<button
						type="button"
						class={`button is-small${isSelected(Button.Custom)}`}
						title="Insert at custom location"
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
