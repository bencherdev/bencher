import type { JsonAuthUser } from "../../types/bencher";
import PerfFrame from "../console/perf/PerfFrame";

export interface Props {
	children?: any;
	apiUrl: string;
	user: JsonAuthUser;
	params: Params;
	isConsole?: boolean;
	isEmbed?: boolean;
	theme: Accessor<Theme>;
	project: Resource<JsonProject>;
	project_slug: Accessor<string | undefined>;
	measuresIsEmpty: Accessor<boolean>;
	branchesIsEmpty: Accessor<boolean>;
	testbedsIsEmpty: Accessor<boolean>;
	benchmarksIsEmpty: Accessor<boolean>;
	isPlotInit: Accessor<boolean>;
	perfQuery: Accessor<JsonPerfQuery>;
	refresh: Accessor<number>;
	measures: Accessor<string[]>;
	start_time: Accessor<string | undefined>;
	end_time: Accessor<string | undefined>;
	start_date: Accessor<string | undefined>;
	end_date: Accessor<string | undefined>;
	key: Accessor<boolean>;
	x_axis: Accessor<XAxis>;
	clear: Accessor<boolean>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	embed_logo: Accessor<boolean>;
	embed_title: Accessor<string | undefined>;
	embed_header: Accessor<boolean>;
	embed_key: Accessor<boolean>;
	handleMeasure: (measure: null | string) => void;
	handleStartTime: (date: string) => void;
	handleEndTime: (date: string) => void;
	handleTab: (tab: PerfTab) => void;
	handleKey: (key: boolean) => void;
	handleXAxis: (x_axis: XAxis) => void;
	handleClear: (clear: boolean) => void;
	handleLowerValue: (end: boolean) => void;
	handleUpperValue: (end: boolean) => void;
	handleLowerBoundary: (boundary: boolean) => void;
	handleUpperBoundary: (boundary: boolean) => void;
}

const PinnedPerf = () => {
	return <PerfFrame />;
};

export default PinnedPerf;
