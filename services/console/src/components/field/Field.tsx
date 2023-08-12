import Input, { InputConfig, InputValue } from "./kinds/Input";
import Checkbox, { CheckboxConfig, CheckboxValue } from "./kinds/Checkbox";
import Switch, { SwitchConfig, SwitchValue } from "./kinds/Switch";
import Select, { SelectConfig, SelectValue } from "./kinds/Select";
import FieldKind from "./kind";
import Radio, { RadioConfig, RadioValue } from "./kinds/Radio";
import type { Params } from "astro";

export type FieldValue =
	| SwitchValue
	| CheckboxValue
	| InputValue
	| SelectValue
	| RadioValue;

export type FieldConfig =
	| SwitchConfig
	| CheckboxConfig
	| InputConfig
	| SelectConfig
	| RadioConfig;

export type FieldHandler = (
	key: string,
	value: FieldValue,
	valid: boolean,
) => void;

export type FieldValueHandler = (value: FieldValue) => void;

export interface Props {
	params?: Params;
	kind: FieldKind;
	fieldKey: string;
	label?: undefined | string;
	value: FieldValue;
	valid: undefined | null | boolean;
	config: FieldConfig;
	handleField: FieldHandler;
}

const Field = (props: Props) => {
	function handleField(value: FieldValue) {
		switch (props.kind) {
			case FieldKind.CHECKBOX:
				props.handleField(props.fieldKey, value, value as CheckboxValue);
				break;
			case FieldKind.SWITCH:
			case FieldKind.RADIO:
				props.handleField(props.fieldKey, value, true);
				break;
			case FieldKind.SELECT:
				props.handleField(
					props.fieldKey,
					{ ...(props.value as SelectValue), selected: value as string },
					true,
				);
				break;
			case FieldKind.INPUT:
			case FieldKind.NUMBER:
				const config = props.config as InputConfig;
				props.handleField(
					props.fieldKey,
					value,
					config.validate ? config.validate(value as string) : true,
				);
				break;
		}
	}

	function getField() {
		switch (props.kind) {
			case FieldKind.CHECKBOX:
				return (
					<Checkbox
						value={props.value as CheckboxValue}
						config={props.config as CheckboxConfig}
						handleField={handleField}
					/>
				);
			case FieldKind.SWITCH:
				return (
					<Switch
						value={props.value as SwitchValue}
						config={props.config as SwitchConfig}
						handleField={handleField}
					/>
				);
			case FieldKind.SELECT:
				return (
					<Select
						value={props.value as SelectValue}
						config={props.config as SelectConfig}
						handleField={handleField}
					/>
				);
			case FieldKind.RADIO:
				return (
					<Radio
						value={props.value as RadioValue}
						config={props.config as RadioConfig}
						params={props.params}
						handleField={handleField}
					/>
				);
			case FieldKind.INPUT:
			case FieldKind.NUMBER:
				return (
					<Input
						value={props.value as InputValue}
						valid={props.valid}
						config={props.config as InputConfig}
						handleField={handleField}
					/>
				);
			default:
				return <div>UNKNOWN FIELD</div>;
		}
	}

	function shouldValidate() {
		switch (props.kind) {
			case FieldKind.CHECKBOX:
			case FieldKind.SWITCH:
			case FieldKind.SELECT:
			case FieldKind.RADIO:
				return false;
			case FieldKind.INPUT:
			case FieldKind.NUMBER:
				return true;
		}
	}

	return (
		<div class="field">
			{props.label && <label class="label is-medium">{props.label}</label>}
			{getField()}
			{shouldValidate() && props.valid === false && (
				<p class="help is-danger">{props.config.help}</p>
			)}
		</div>
	);
};

export default Field;
