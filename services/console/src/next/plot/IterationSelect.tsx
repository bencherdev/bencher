import { For } from "solid-js";

/**
 * Every iteration of each report, or only one, counting from 1. Its place is
 * held while no report ran a second, so its arrival moves nothing.
 */
const IterationSelect = (props: {
	/** The most iterations any report drawn ran. */
	count: number;
	value: number | undefined;
	onChange: (iteration: number | undefined) => void;
}) => {
	return (
		<span class="iter" classList={{ "iter-idle": props.count < 2 }}>
			<span class="eyebrow" aria-hidden="true">
				Iteration
			</span>
			<span class="ui-select-wrap">
				<select
					class="ui-select"
					data-size="sm"
					aria-label="Iteration"
					onChange={(event) => {
						const { value } = event.currentTarget;
						props.onChange(value === "all" ? undefined : Number(value));
					}}
				>
					<option value="all" selected={props.value === undefined}>
						All
					</option>
					<For each={Array.from({ length: props.count }, (_, at) => at + 1)}>
						{(iteration) => (
							<option value={iteration} selected={props.value === iteration}>
								{iteration}
							</option>
						)}
					</For>
				</select>
			</span>
		</span>
	);
};

export default IterationSelect;
