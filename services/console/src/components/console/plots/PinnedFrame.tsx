import { type Accessor, createMemo } from "solid-js";
import type { PerfTab } from "../../../config/types";
import {
	type JsonAuthUser,
	type JsonPerfQuery,
	type JsonPlot,
	XAxis,
} from "../../../types/bencher";
import { timeToDateOnlyIso } from "../../../util/convert";
import { themeSignal } from "../../navbar/theme/util";
import PerfFrame from "../perf/PerfFrame";

export interface Props {
	isConsole: boolean;
	apiUrl: string;
	user: JsonAuthUser;
	project_slug: Accessor<undefined | string>;
	plot: JsonPlot;
}

const PinnedFrame = (props: Props) => {
	const theme = themeSignal;

	const measuresIsEmpty = createMemo(() => false);
	const branchesIsEmpty = createMemo(() => false);
	const testbedsIsEmpty = createMemo(() => false);
	const benchmarksIsEmpty = createMemo(() => false);
	const isPlotInit = createMemo(() => false);

	const start_time = createMemo(() =>
		(Date.now() - (props.plot?.window ?? 0) * 1_000).toString(),
	);
	const end_time = createMemo(() => Date.now().toString());

	const perfQuery = createMemo(() => {
		return {
			branches: props.plot?.branches ?? [],
			heads: [],
			testbeds: props.plot?.testbeds ?? [],
			benchmarks: props.plot?.benchmarks ?? [],
			measures: props.plot?.measures ?? [],
			start_time: start_time(),
			end_time: end_time(),
		} as JsonPerfQuery;
	});

	const refresh = createMemo(() => 0);

	const measures = createMemo(() => props.plot?.measures ?? []);

	const start_date = createMemo(() => timeToDateOnlyIso(start_time()));
	const end_date = createMemo(() => timeToDateOnlyIso(end_time()));

	const key = createMemo(() => false);
	const x_axis = createMemo(() => props.plot?.x_axis ?? XAxis.DateTime);
	const clear = createMemo(() => false);

	const lower_value = createMemo(() => props.plot?.lower_value ?? false);
	const upper_value = createMemo(() => props.plot?.upper_value ?? false);
	const lower_boundary = createMemo(() => props.plot?.lower_boundary ?? false);
	const upper_boundary = createMemo(() => props.plot?.upper_boundary ?? false);

	const embed_logo = createMemo(() => false);
	const embed_title = createMemo(() => props.plot?.title ?? "");
	const embed_header = createMemo(() => false);
	const embed_key = createMemo(() => false);

	const handleVoid = (_void: string | PerfTab | boolean | XAxis | null) => {};

	return (
		<div id={`plot-${props.plot?.uuid}`}>
			<PerfFrame
				apiUrl={props.apiUrl}
				user={props.user}
				isConsole={props.isConsole}
				isEmbed={true}
				theme={theme}
				project_slug={props.project_slug}
				measuresIsEmpty={measuresIsEmpty}
				branchesIsEmpty={branchesIsEmpty}
				testbedsIsEmpty={testbedsIsEmpty}
				benchmarksIsEmpty={benchmarksIsEmpty}
				isPlotInit={isPlotInit}
				perfQuery={perfQuery}
				refresh={refresh}
				measures={measures}
				start_date={start_date}
				end_date={end_date}
				key={key}
				x_axis={x_axis}
				clear={clear}
				lower_value={lower_value}
				upper_value={upper_value}
				lower_boundary={lower_boundary}
				upper_boundary={upper_boundary}
				embed_logo={embed_logo}
				embed_title={embed_title}
				embed_header={embed_header}
				embed_key={embed_key}
				handleMeasure={handleVoid}
				handleStartTime={handleVoid}
				handleEndTime={handleVoid}
				handleTab={handleVoid}
				handleKey={handleVoid}
				handleXAxis={handleVoid}
				handleClear={handleVoid}
				handleLowerValue={handleVoid}
				handleUpperValue={handleVoid}
				handleLowerBoundary={handleVoid}
				handleUpperBoundary={handleVoid}
			/>
		</div>
	);
};

export default PinnedFrame;
