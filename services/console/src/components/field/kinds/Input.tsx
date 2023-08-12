import type { FieldValueHandler } from "../Field";

export type InputValue = string | number | null | undefined;

export interface Props {
	value: InputValue;
	valid: undefined | null | boolean;
	config: InputConfig;
	handleField: FieldValueHandler;
}

export interface InputConfig {
	icon: string;
	type: string;
	placeholder?: string;
	value: InputValue;
	disabled?: boolean;
	help?: string;
	validate: (value: InputValue) => boolean;
}

const Input = (props: Props) => {
	return (
		<div class="control has-icons-left has-icons-right">
			<span class="icon is-small is-left">
				<i class={props.config.icon} />
			</span>
			<input
				class="input"
				type={props.config.type}
				placeholder={props.config.placeholder as string}
				value={props.value as string}
				disabled={props.config.disabled as boolean}
				onInput={(event) => props.handleField(event.target?.value)}
			/>
			{props.valid && (
				<span class="icon is-small is-right">
					<i class="fas fa-check" />
				</span>
			)}
		</div>
	);
};

export default Input;
