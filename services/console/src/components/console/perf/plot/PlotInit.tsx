import type { Accessor } from "solid-js";
import { PerfTab } from "../../../../config/types";

export interface Props {
	metric_kind: Accessor<undefined | string>;
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
						checked={props.metric_kind() ? true : false}
						disabled={true}
					/>
					Select a Metric Kind
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
						onClick={() => {
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
						onClick={() => {
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
						onClick={() => {
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
