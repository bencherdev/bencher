import { createElementSize } from "@solid-primitives/resize-observer";
import {
	type Accessor,
	type Resource,
	createMemo,
	createResource,
	Show,
} from "solid-js";
import { createStore } from "solid-js/store";
import type { PerfRange } from "../../../../config/types";
import type { JsonPerf } from "../../../../types/bencher";
import LinePlot from "./line/LinePlot";
import PlotKey from "./key/PlotKey";
import type { Theme } from "../../../navbar/theme/theme";

export interface Props {
	theme: Accessor<Theme>;
	isConsole: boolean;
	isEmbed: boolean;
	range: Accessor<PerfRange>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	perfData: Resource<JsonPerf>;
	key: Accessor<boolean>;
	embed_key: Accessor<boolean>;
	handleKey: (key: boolean) => void;
}

const Plot = (props: Props) => {
	const [perfActive, setPerfActive] = createStore<boolean[]>([]);

	const [_active] = createResource(props.perfData, (json_perf) => {
		const active: boolean[] = [];
		if (json_perf?.results) {
			for (const _ of json_perf.results) {
				active.push(true);
			}
		}
		setPerfActive(active);
		return active;
	});

	const handlePerfActive = (index: number) => {
		const active = [...perfActive];
		active[index] = !active[index];
		setPerfActive(active);
	};

	const togglePerfActive = () => {
		const allActive = perfActive.reduce((acc, curr) => {
			return acc && curr;
		}, true);
		const active = perfActive.map(() => !allActive);
		setPerfActive(active);
	};

	let plot_ref: HTMLDivElement | undefined;
	const plot_size = createElementSize(() => plot_ref);
	const width = createMemo(() => plot_size.width ?? 100);

	return (
		<div class="container">
			<div
				ref={(e) => {
					plot_ref = e;
				}}
			>
				<LinePlot
					theme={props.theme}
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
			<Show when={!props.isEmbed || props.embed_key()}>
				<PlotKey
					perfData={props.perfData}
					key={props.key}
					handleKey={props.handleKey}
					perfActive={perfActive}
					handlePerfActive={handlePerfActive}
					togglePerfActive={togglePerfActive}
				/>
			</Show>
		</div>
	);
};

export default Plot;
