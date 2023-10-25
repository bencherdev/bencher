import type { FieldValueHandler, FieldValue } from "../Field";

export type SwitchValue = boolean;

export interface Props {
	value: FieldValue;
	config: SwitchConfig;
	handleField: FieldValueHandler;
}

export interface SwitchConfig {
	label: string;
	disabled?: boolean;
	help?: string;
}

const Switch = (props: Props) => {
	return (
		<div class="field" id={props.config?.label}>
			<input
				id={props.config?.label}
				type="checkbox"
				class="switch"
				name={props.config?.label}
				checked={props.value as boolean}
				disabled={props.config?.disabled as boolean}
			/>
			{/* biome-ignore lint/a11y/useKeyWithClickEvents: TODO */}
			<label
				for={props.config?.label}
				onClick={(_event) => {
					if (props.config?.disabled) {
						return;
					}
					props.handleField(!props.value);
				}}
			/>
		</div>
	);
};

export default Switch;
