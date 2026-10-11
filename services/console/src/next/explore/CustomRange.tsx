import TextInput from "@bencherdev/ui/TextInput";
import { createUniqueId } from "solid-js";
import type { QueryWindow } from "../query/query";
import { customRange, dateValue } from "./window";

/** A window from one local day through another. */
const CustomRange = (props: {
	window: QueryWindow & { start: number };
	now: number;
	onWindow: (window: QueryWindow) => void;
}) => {
	const from = createUniqueId();
	const to = createUniqueId();
	const end = () => props.window.end ?? props.now;
	const change = (first: string, last: string) => {
		const window = customRange(first, last);
		if (window) {
			props.onWindow(window);
		}
	};
	return (
		<fieldset class="ex-range">
			<legend class="sr-only">Custom window</legend>
			<label class="eyebrow" for={from}>
				From
			</label>
			<TextInput
				id={from}
				type="date"
				size="sm"
				value={dateValue(props.window.start)}
				max={dateValue(end())}
				onChange={(event) =>
					change(event.currentTarget.value, dateValue(end()))
				}
			/>
			<label class="eyebrow" for={to}>
				To
			</label>
			<TextInput
				id={to}
				type="date"
				size="sm"
				value={dateValue(end())}
				min={dateValue(props.window.start)}
				onChange={(event) =>
					change(dateValue(props.window.start), event.currentTarget.value)
				}
			/>
		</fieldset>
	);
};

export default CustomRange;
