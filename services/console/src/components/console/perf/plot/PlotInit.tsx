import type { Accessor } from "solid-js";
import { PerfTab } from "../../../../config/types";
import { BENCHER_MEASURE_ID } from "./util";

export interface Props {
	measures: Accessor<string[]>;
	branches: Accessor<string[]>;
	testbeds: Accessor<string[]>;
	benchmarks: Accessor<string[]>;
	handleTab: (tab: PerfTab) => void;
}

const PlotInit = (props: Props) => {
	return (
		<div class="content">
			<ul>
				<li class="checkbox">
					<input
						type="checkbox"
						checked={props.measures().length > 0}
						disabled={true}
					/>
					Select a <a href={`#${BENCHER_MEASURE_ID}`}>Measure</a>
				</li>
				<br />
				<li class="checkbox">
					<input
						type="checkbox"
						checked={props.branches().length > 0}
						disabled={true}
					/>
					Select at least one{" "}
					<a
						title="View Branches"
						// biome-ignore lint/a11y/useValidAnchor: stateful anchor
						onClick={(_e) => {
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
						checked={props.testbeds().length > 0}
						disabled={true}
					/>
					Select at least one{" "}
					<a
						title="View Testbeds"
						// biome-ignore lint/a11y/useValidAnchor: stateful anchor
						onClick={(_e) => {
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
						checked={props.benchmarks().length > 0}
						disabled={true}
					/>
					Select at least one{" "}
					<a
						title="View Benchmarks"
						// biome-ignore lint/a11y/useValidAnchor: stateful anchor
						onClick={(_e) => {
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
