import type { JSX } from "solid-js";
import { For, splitProps } from "solid-js";

export interface SegmentedProps<T extends string>
	extends Omit<JSX.HTMLAttributes<HTMLDivElement>, "onChange"> {
	/** The radios' shared name, unique on the page. */
	name: string;
	options: readonly { value: T; label: JSX.Element }[];
	/** The checked position; with none, nothing is checked. */
	value: T | undefined;
	onChange: (value: T) => void;
	/** Stretch to the container, positions sharing the width. */
	full?: boolean;
}

/**
 * One choice among a few, drawn as joined positions. Each position is a
 * native radio, so the arrow keys move the choice and a form reads it. Label
 * the group with `aria-label` or `aria-labelledby`.
 */
const Segmented = <T extends string>(props: SegmentedProps<T>) => {
	const [local, rest] = splitProps(props, [
		"name",
		"options",
		"value",
		"onChange",
		"full",
		"class",
	]);
	return (
		<div
			role="radiogroup"
			class={
				local.class === undefined
					? "ui-segmented"
					: `ui-segmented ${local.class}`
			}
			data-full={local.full === true ? "" : undefined}
			{...rest}
		>
			<For each={local.options}>
				{(option) => (
					<label class="ui-segment">
						<input
							type="radio"
							name={local.name}
							value={option.value}
							checked={option.value === local.value}
							onChange={() => local.onChange(option.value)}
						/>
						{option.label}
					</label>
				)}
			</For>
		</div>
	);
};

export default Segmented;
