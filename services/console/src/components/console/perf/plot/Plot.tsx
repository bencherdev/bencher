import { createElementSize } from "@solid-primitives/resize-observer";
import {
	type Accessor,
	type Resource,
	createMemo,
	createResource,
} from "solid-js";
import { createStore } from "solid-js/store";
import type { PerfRange } from "../../../../config/types";
import type { JsonPerf } from "../../../../types/bencher";
import LinePlot from "./LinePlot";
import PlotKey from "./PlotKey";

export interface Props {
	isConsole: boolean;
	range: Accessor<PerfRange>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	perfData: Resource<JsonPerf>;
	key: Accessor<boolean>;
	handleKey: (key: boolean) => void;
}

const Plot = (props: Props) => {
	const [perfActive, setPerfActive] = createStore<boolean[]>([]);

	const [_active] = createResource(props.perfData, (json_perf) => {
		const active: boolean[] = [];
		json_perf?.results?.forEach(() => {
			active.push(true);
		});
		setPerfActive(active);
		return active;
	});

	const handlePerfActive = (index: number) => {
		const active = [...perfActive];
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
					isConsole={props.isConsole}
					perfData={props.perfData}
					range={props.range}
					lower_value={props.lower_value}
					upper_value={props.upper_value}
					lower_boundary={props.lower_boundary}
					upper_boundary={props.upper_boundary}
					perfActive={perfActive}
					width={width}
				/>
			</div>
			<br />
			<PlotKey
				perfData={props.perfData}
				key={props.key}
				handleKey={props.handleKey}
				perfActive={perfActive}
				handlePerfActive={handlePerfActive}
			/>
		</div>
	);
};

export default Plot;
