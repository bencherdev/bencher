import { PerfTab } from "../../../config/types";

const PlotInit = (props) => {
	return (
		<div class="content">
			<ul>
				<li class="checkbox">
					<input
						type="checkbox"
						checked={props.metric_kind()}
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
