import { createElementSize } from "@solid-primitives/resize-observer";
import { Accessor, createMemo, createResource } from "solid-js";
import { createStore } from "solid-js/store";
import LinePlot from "./LinePlot";
import PlotKey from "./PlotKey";
import type { JsonAuthUser, JsonPerf } from "../../../../types/bencher";
import type { PerfPlotConfig } from "./PerfPlot";
import type { PerfRange } from "../../../../config/types";

export interface Props {
	user: JsonAuthUser;
	config: PerfPlotConfig;
	range: Accessor<PerfRange>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	perfData: Accessor<JsonPerf>;
	key: Accessor<boolean>;
	handleKey: (key: boolean) => void;
}

const Plot = (props: Props) => {
	const [perf_active, setPerfActive] = createStore<boolean[]>([]);

	const [_perf_active] = createResource(props.perfData, (json_perf) => {
		const active: boolean[] = [];
		json_perf?.results?.forEach(() => {
			active.push(true);
		});
		setPerfActive(active);
		return active;
	});

	const handlePerfActive = (index: number) => {
		const active = [...perf_active];
		active[index] = !active[index];
		setPerfActive(active);
	};

	let plot_ref: HTMLDivElement | undefined;
	const plot_size = createElementSize(() => plot_ref);
	const width = createMemo(() => plot_size.width ?? 100);

	return (
		<div class="container">
			<div ref={(e) => (plot_ref = e)}>
				<LinePlot
					user={props.user}
					config={props.config}
					perfData={props.perfData}
					range={props.range}
					lower_boundary={props.lower_boundary}
					upper_boundary={props.upper_boundary}
					perf_active={perf_active}
					width={width}
				/>
			</div>
			<br />
			<PlotKey
				perfData={props.perfData}
				key={props.key}
				perf_active={perf_active}
				handleKey={props.handleKey}
				handlePerfActive={handlePerfActive}
			/>
		</div>
	);
};

export default Plot;
