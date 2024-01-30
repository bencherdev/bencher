import type { FieldValueHandler } from "../Field";

export type SearchValue = string | number | null | undefined;

export interface Props {
	value: SearchValue;
	valid: undefined | null | boolean;
	config: SearchConfig;
	handleField: FieldValueHandler;
}

export interface SearchConfig {
	icon: string;
	type: string;
	placeholder?: string;
	value: SearchValue;
}

const Search = (props: Props) => {
	return (
		<div class="control has-icons-left has-icons-right">
			<span class="icon is-small is-left">
				<i class={props.config.icon} />
			</span>
			<input
				class="input"
				type="search"
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

export default Search;
