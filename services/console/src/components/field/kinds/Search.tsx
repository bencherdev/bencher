import type { FieldValueHandler } from "../Field";

export type SearchValue = string | number | null | undefined;

export interface Props {
	value: SearchValue;
	config: SearchConfig;
	handleField: FieldValueHandler;
}

export interface SearchConfig {
	placeholder: string;
}

const Search = (props: Props) => {
	return (
		<div class="control has-icons-left has-icons-right">
			<span class="icon is-small is-left">
				<i class="fas fa-search" />
			</span>
			<input
				class="input"
				type="search"
				placeholder={props.config.placeholder as string}
				value={props.value as string}
				onInput={(event) => props.handleField(event.target?.value)}
			/>
		</div>
	);
};

export default Search;
