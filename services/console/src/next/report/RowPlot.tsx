import { createSignal } from "solid-js";
import Plot from "../plot/Plot";
import type { PlotData } from "../plot/types";

/** A row's full plot, in its own chunk so a report draws its rows before the plot's code arrives. */
const RowPlot = (props: {
	data: PlotData;
	note: string;
	reportHref: (uuid: string) => string;
}) => {
	const [hidden, setHidden] = createSignal<string[]>([]);
	const [focused, setFocused] = createSignal<string | null>();
	return (
		<Plot
			data={props.data}
			hidden={hidden()}
			focused={focused()}
			onHiddenChange={setHidden}
			onFocusChange={setFocused}
			size="row"
			note={props.note}
			reportHref={props.reportHref}
		/>
	);
};

export default RowPlot;
