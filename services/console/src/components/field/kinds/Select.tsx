import type { FieldValueHandler } from "../Field";

export interface Props {
	value: SelectValue;
	config: SelectConfig;
	handleField: FieldValueHandler;
}

export interface SelectConfig {
	icon: string;
	type: string;
	placeholder?: string;
	value: SelectValue;
	disabled?: boolean;
	help?: string;
	validate: (value: SelectValue) => boolean;
}

export interface SelectValue {
	selected: string;
	options: SelectOption[];
}

export interface SelectOption {
	value: string;
	option: string;
}

const Select = (props: Props) => {
	return (
		<nav class="level is-mobile">
			<div class="level-left">
				<div class="level-item">
					<div class="control has-icons-left">
						<div class="icon is-small is-left">
							<i class={props.config.icon} />
						</div>
						<div class="select">
							<select
								value={props.value.selected}
								onChange={(event) =>
									props.handleField(event.currentTarget?.value)
								}
							>
								{props.value.options.map((option) => {
									return (
										<option id={option.value} value={option.value}>
											{option.option}
										</option>
									);
								})}
							</select>
						</div>
					</div>
				</div>
			</div>
		</nav>
	);
};

export default Select;
