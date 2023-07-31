import { FieldValueHandler } from "../Field";

export type InputValue = string | number | null | undefined;

export interface InputConfig {
	icon: string;
	type: string;
	placeholder?: string;
	value: InputValue;
	disabled?: boolean;
	validate: (value: InputValue) => boolean;
}

const Input = (props: {
	value: InputValue;
	valid: boolean;
	config: InputConfig;
	handleField: FieldValueHandler;
}) => {
	return (
		<div class="control has-icons-left has-icons-right">
			<span class="icon is-small is-left">
				<i class={props.config.icon} />
			</span>
			<input
				class="input"
				type={props.config.type}
				placeholder={props.config.placeholder}
				value={props.value}
				disabled={props.config.disabled}
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
