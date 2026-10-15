import { createSignal } from "solid-js";
import { useAlertChange } from "../alerts/change";
import type { Settable } from "../alerts/status";
import Plot, { type PlotAlerts } from "../plot/Plot";
import type { PlotData } from "../plot/types";

/** A row's full plot, in its own chunk so a report draws its rows before the plot's code arrives. */
const RowPlot = (props: {
	data: PlotData;
	note: string;
	reportHref: (uuid: string) => string;
	alerts?: PlotAlerts | undefined;
	kept?: Map<string, Settable> | undefined;
	canChange?: (() => boolean) | undefined;
}) => {
	const [hidden, setHidden] = createSignal<string[]>([]);
	const [focused, setFocused] = createSignal<string | null>();
	// A page that leaves its row plots to change alerts has their code arrive with the plot.
	const alerts =
		props.alerts ??
		(props.canChange && useAlertChange(props.kept).readout(props.canChange));
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
			alerts={alerts}
		/>
	);
};

export default RowPlot;
