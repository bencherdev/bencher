import Input, { type InputConfig, type InputValue } from "./kinds/Input";
import Checkbox, {
	type CheckboxConfig,
	type CheckboxValue,
} from "./kinds/Checkbox";
import Switch, { type SwitchConfig, type SwitchValue } from "./kinds/Switch";
import Select, { type SelectConfig, type SelectValue } from "./kinds/Select";
import FieldKind from "./kind";
import Radio, { type RadioConfig, type RadioValue } from "./kinds/Radio";
import type { Params } from "astro";
import Model from "./kinds/Model";
import { validModel, validIndex, validWindow } from "../../util/valid";
import type { SearchConfig, SearchValue } from "./kinds/Search";
import Search from "./kinds/Search";
import type { PlotRankConfig, PlotRankValue } from "./kinds/PlotRank";
import PlotRank from "./kinds/PlotRank";
import type { PlotWindowConfig, PlotWindowValue } from "./kinds/PlotWindow";
import PlotWindow from "./kinds/PlotWindow";

export type FieldValue =
	| SwitchValue
	| CheckboxValue
	| InputValue
	| SelectValue
	| RadioValue
	| SearchValue
	| PlotWindowValue
	| PlotRankValue;

export type FieldConfig =
	| SwitchConfig
	| CheckboxConfig
	| InputConfig
	| SelectConfig
	| RadioConfig
	| SearchConfig
	| PlotWindowConfig
	| PlotRankConfig;

export type FieldHandler = (
	key: string,
	value: FieldValue,
	valid: boolean,
) => void;

export type FieldValueHandler = (value: FieldValue) => void;

export interface Props {
	apiUrl?: string;
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
	const handleField = (value: FieldValue) => {
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
			case FieldKind.NUMBER: {
				const config = props.config as InputConfig;
				props.handleField(
					props.fieldKey,
					value,
					config.validate ? config.validate(value as string) : true,
				);
				break;
			}
			case FieldKind.MODEL:
				props.handleField(props.fieldKey, value, validModel(value));
				break;
			case FieldKind.SEARCH:
				props.handleField(props.fieldKey, value, true);
				break;
			case FieldKind.PLOT_WINDOW:
				props.handleField(props.fieldKey, value, validWindow(value));
				break;
			case FieldKind.PLOT_RANK:
				props.handleField(props.fieldKey, value, validIndex(value));
				break;
		}
	};

	const getField = () => {
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
						apiUrl={props.apiUrl as string}
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
			case FieldKind.MODEL:
				return (
					<Model
						value={props.value as InputValue}
						valid={props.valid}
						config={props.config as InputConfig}
						handleField={handleField}
					/>
				);
			case FieldKind.SEARCH:
				return (
					<Search
						value={props.value as SearchValue}
						config={props.config as SearchConfig}
						handleField={handleField}
					/>
				);
			case FieldKind.PLOT_WINDOW:
				return (
					<PlotWindow
						value={props.value as PlotWindowValue}
						valid={props.valid}
						config={props.config as PlotWindowConfig}
						handleField={handleField}
					/>
				);
			case FieldKind.PLOT_RANK:
				return (
					<PlotRank
						value={props.value as PlotRankValue}
						valid={props.valid}
						config={props.config as PlotRankConfig}
						handleField={handleField}
					/>
				);
			default:
				return <div>UNKNOWN FIELD</div>;
		}
	};

	const shouldValidate = () => {
		switch (props.kind) {
			case FieldKind.CHECKBOX:
			case FieldKind.SWITCH:
			case FieldKind.SELECT:
			case FieldKind.RADIO:
			case FieldKind.SEARCH:
				return false;
			case FieldKind.INPUT:
			case FieldKind.NUMBER:
			case FieldKind.MODEL:
			case FieldKind.PLOT_WINDOW:
			case FieldKind.PLOT_RANK:
				return true;
		}
	};

	return (
		<div class="field">
			{props.label && <label class="label is-medium">{props.label}</label>}
			{getField()}
			{shouldValidate() && props.valid === false && (
				<p class="help is-danger">{props.config?.help}</p>
			)}
		</div>
	);
};

export default Field;
