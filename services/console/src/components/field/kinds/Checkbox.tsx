import { JSX } from "solid-js";
import { FieldValueHandler } from "../Field";

export type CheckboxValue = boolean;

export interface CheckboxConfig {
	label: string;
	placeholder: JSX.Element;
}

const Checkbox = (props: {
	value: CheckboxValue;
	config: CheckboxConfig;
	handleField: FieldValueHandler;
}) => {
	return (
		<div class="field" id={props.config.label}>
			<input
				id={props.config.label}
				type="checkbox"
				name={props.config.label}
				class="is-checkradio is-small"
				checked={props.value}
				onInput={(event) => props.handleField(event.target?.checked)}
			/>
			<label for={props.config.label}>
				<small>{props.config.placeholder}</small>
			</label>
		</div>
	);
};

export default Checkbox;
