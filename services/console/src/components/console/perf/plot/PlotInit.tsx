import type { Accessor } from "solid-js";
import { PerfTab } from "../../../../config/types";
import { BENCHER_MEASURE_ID } from "./util";

export interface Props {
	measuresIsEmpty: Accessor<boolean>;
	branchesIsEmpty: Accessor<boolean>;
	testbedsIsEmpty: Accessor<boolean>;
	benchmarksIsEmpty: Accessor<boolean>;
	handleTab: (tab: PerfTab) => void;
}

const PlotInit = (props: Props) => {
	return (
		<div class="content">
			<ul>
				<li class="checkbox">
					<input
						type="checkbox"
						checked={!props.measuresIsEmpty()}
						disabled={true}
					/>
					Select a <a href={`#${BENCHER_MEASURE_ID}`}>Measure</a>
				</li>
				<br />
				<li class="checkbox">
					<input
						type="checkbox"
						checked={!props.branchesIsEmpty()}
						disabled={true}
					/>
					Select at least one{" "}
					{/* biome-ignore lint/a11y/useValidAnchor: action on press */}
					<a
						title="View Branches"
						onMouseDown={(_e) => {
							props.handleTab(PerfTab.BRANCHES);
						}}
					>
						Branch
					</a>
				</li>
				<br />
				<li class="checkbox">
					<input
						type="checkbox"
						checked={!props.testbedsIsEmpty()}
						disabled={true}
					/>
					Select at least one{" "}
					{/* biome-ignore lint/a11y/useValidAnchor: action on press */}
					<a
						title="View Testbeds"
						onMouseDown={(_e) => {
							props.handleTab(PerfTab.TESTBEDS);
						}}
					>
						Testbed
					</a>
				</li>
				<br />
				<li class="checkbox">
					<input
						type="checkbox"
						checked={!props.benchmarksIsEmpty()}
						disabled={true}
					/>
					Select at least one{" "}
					{/* biome-ignore lint/a11y/useValidAnchor: action on press */}
					<a
						title="View Benchmarks"
						onMouseDown={(_e) => {
							props.handleTab(PerfTab.BENCHMARKS);
						}}
					>
						Benchmark
					</a>
				</li>
			</ul>
			<br />
		</div>
	);
};

export default PlotInit;
