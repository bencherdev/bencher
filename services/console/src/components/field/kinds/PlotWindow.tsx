import { createMemo } from "solid-js";
import type { FieldValueHandler } from "../Field";

export type PlotWindowValue = number | null | undefined;

export interface Props {
	value: PlotWindowValue;
	valid: undefined | null | boolean;
	config: PlotWindowConfig;
	handleField: FieldValueHandler;
}

export interface PlotWindowConfig {
	help: string;
}

// 60 seconds * 60 minutes * 24 hours * 7 days
enum Button {
	FourWeeks = 60 * 60 * 24 * 7 * 4,
	EightWeeks = 60 * 60 * 24 * 7 * 8,
	TwelveWeeks = 60 * 60 * 24 * 7 * 12,
	Custom = 0,
}

const PlotWindow = (props: Props) => {
	const isSelected = (button: Button) => {
		const IS_SELECTED = " is-primary is-selected";
		switch (button) {
			case Button.FourWeeks:
			case Button.EightWeeks:
			case Button.TwelveWeeks:
				return button === props.value ? IS_SELECTED : "";
			case Button.Custom:
				return props.value !== Button.FourWeeks &&
					props.value !== Button.EightWeeks &&
					props.value !== Button.TwelveWeeks
					? IS_SELECTED
					: "";
		}
	};

	const rank = createMemo(() => {
		switch (props.value) {
			case Button.FourWeeks:
			case Button.EightWeeks:
			case Button.TwelveWeeks:
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
					class={`button is-small${isSelected(Button.FourWeeks)}`}
					title={props.config?.bottom}
					onMouseDown={(e) => {
						e.preventDefault();
						props.handleField(Button.FourWeeks);
					}}
				>
					4 Weeks
				</button>
				<button
					type="button"
					class={`button is-small${isSelected(Button.EightWeeks)}`}
					title={props.config?.bottom}
					onMouseDown={(e) => {
						e.preventDefault();
						props.handleField(Button.EightWeeks);
					}}
				>
					8 Weeks
				</button>
				<button
					type="button"
					class={`button is-small${isSelected(Button.TwelveWeeks)}`}
					title={props.config?.bottom}
					onMouseDown={(e) => {
						e.preventDefault();
						props.handleField(Button.TwelveWeeks);
					}}
				>
					12 Weeks
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

export default PlotWindow;
