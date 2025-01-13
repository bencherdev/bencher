import {
	type Accessor,
	Match,
	type Resource,
	Switch,
	createMemo,
} from "solid-js";
import type { PerfTab } from "../../../../config/types";
import type {
	JsonAuthUser,
	JsonPerf,
	JsonProject,
	XAxis,
} from "../../../../types/bencher";
import { type Theme, themeColor } from "../../../navbar/theme/theme";
import Plot from "./Plot";
import PlotHeader from "./PlotHeader";
import PlotInit from "./PlotInit";

export interface Props {
	children: Element | undefined;
	apiUrl: string;
	user: JsonAuthUser;
	project: Resource<JsonProject> | undefined;
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	isEmbed: boolean;
	plotId: string | undefined;
	measuresIsEmpty: Accessor<boolean>;
	branchesIsEmpty: Accessor<boolean>;
	testbedsIsEmpty: Accessor<boolean>;
	benchmarksIsEmpty: Accessor<boolean>;
	isPlotInit: Accessor<boolean>;
	refresh: () => void;
	perfData: Resource<JsonPerf>;
	measures: Accessor<string[]>;
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	key: Accessor<boolean>;
	x_axis: Accessor<XAxis>;
	clear: Accessor<boolean>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	embed_logo: Accessor<boolean>;
	embed_title: Accessor<undefined | string>;
	embed_header: Accessor<boolean>;
	embed_key: Accessor<boolean>;
	handleMeasure: (index: number, slug: null | string) => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleTab: (tab: PerfTab) => void;
	handleKey: (key: boolean) => void;
	handleXAxis: (x_axis: XAxis) => void;
	handleClear: (clear: boolean) => void;
	handleLowerValue: (lower_value: boolean) => void;
	handleUpperValue: (upper_value: boolean) => void;
	handleLowerBoundary: (lower_boundary: boolean) => void;
	handleUpperBoundary: (upper_boundary: boolean) => void;
}

const PerfPlot = (props: Props) => {
	const themeClass = createMemo(() => themeColor(props.theme()));

	return (
		<div class="columns">
			<div class="column">
				<div class={`panel ${themeClass()}`}>
					<PlotHeader
						apiUrl={props.apiUrl}
						user={props.user}
						project={props.project}
						project_slug={props.project_slug}
						theme={props.theme}
						isConsole={props.isConsole}
						isEmbed={props.isEmbed}
						isPlotInit={props.isPlotInit}
						measures={props.measures}
						start_date={props.start_date}
						end_date={props.end_date}
						refresh={props.refresh}
						x_axis={props.x_axis}
						clear={props.clear}
						lower_value={props.lower_value}
						upper_value={props.upper_value}
						lower_boundary={props.lower_boundary}
						upper_boundary={props.upper_boundary}
						embed_logo={props.embed_logo}
						embed_title={props.embed_title}
						embed_header={props.embed_header}
						handleMeasure={props.handleMeasure}
						handleStartTime={props.handleStartTime}
						handleEndTime={props.handleEndTime}
						handleXAxis={props.handleXAxis}
						handleClear={props.handleClear}
						handleLowerValue={props.handleLowerValue}
						handleUpperValue={props.handleUpperValue}
						handleLowerBoundary={props.handleLowerBoundary}
						handleUpperBoundary={props.handleUpperBoundary}
					/>
					<div class="panel-block">
						<Switch
							fallback={
								<Plot
									theme={props.theme}
									isConsole={props.isConsole}
									isEmbed={props.isEmbed}
									plotId={props.plotId}
									measures={props.measures}
									x_axis={props.x_axis}
									lower_value={props.lower_value}
									upper_value={props.upper_value}
									lower_boundary={props.lower_boundary}
									upper_boundary={props.upper_boundary}
									perfData={props.perfData}
									key={props.key}
									embed_key={props.embed_key}
									handleKey={props.handleKey}
								/>
							}
						>
							<Match when={props.perfData.loading}>
								<progress
									class="progress is-primary"
									style="margin-top: 8rem; margin-bottom: 16rem;"
									max="100"
								/>
							</Match>
							<Match when={props.isPlotInit()}>
								<PlotInit
									measuresIsEmpty={props.measuresIsEmpty}
									branchesIsEmpty={props.branchesIsEmpty}
									testbedsIsEmpty={props.testbedsIsEmpty}
									benchmarksIsEmpty={props.benchmarksIsEmpty}
									handleTab={props.handleTab}
								/>
							</Match>
						</Switch>
					</div>
					{props.children}
				</div>
			</div>
		</div>
	);
};

export default PerfPlot;
