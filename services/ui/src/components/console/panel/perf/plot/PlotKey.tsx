import axios from "axios";
import { createMemo, createResource, For } from "solid-js";
import { PerfTab } from "../../../config/types";
import * as d3 from "d3";
import { get_options, validate_jwt } from "../../../../site/util";

const PlotKey = (props) => {
	const branches_fetcher = createMemo(() => {
		return {
			branches: props.branches(),
			token: props.user?.token,
		};
	});
	const testbeds_fetcher = createMemo(() => {
		return {
			testbeds: props.testbeds(),
			token: props.user?.token,
		};
	});
	const benchmarks_fetcher = createMemo(() => {
		return {
			benchmarks: props.benchmarks(),
			token: props.user?.token,
		};
	});

	const getOne = async (perf_tab: PerfTab, fetcher) => {
		const key_data = {};

		await Promise.all(
			fetcher[perf_tab]?.map(async (uuid: string) => {
				try {
					const url = props.config?.key_url(
						props.path_params(),
						perf_tab,
						uuid,
					);
					const resp = await axios(get_options(url, fetcher.token));
					key_data[uuid] = resp.data;
				} catch (error) {
					console.error(error);
				}
			}),
		);

		return key_data;
	};

	const [branches] = createResource(branches_fetcher, async (fetcher) => {
		return getOne(PerfTab.BRANCHES, fetcher);
	});
	const [testbeds] = createResource(testbeds_fetcher, async (fetcher) => {
		return getOne(PerfTab.TESTBEDS, fetcher);
	});
	const [benchmarks] = createResource(benchmarks_fetcher, async (fetcher) => {
		return getOne(PerfTab.BENCHMARKS, fetcher);
	});

	return (
		<>
			{props.key() ? (
				<ExpandedKey
					branches={branches}
					testbeds={testbeds}
					benchmarks={benchmarks}
					perf_data={props.perf_data}
					perf_active={props.perf_active}
					handleKey={props.handleKey}
					handlePerfActive={props.handlePerfActive}
				/>
			) : (
				<MinimizedKey
					perf_data={props.perf_data}
					perf_active={props.perf_active}
					handleKey={props.handleKey}
					handlePerfActive={props.handlePerfActive}
				/>
			)}
		</>
	);
};

const ExpandedKey = (props) => {
	return (
		<>
			<MinimizeKeyButton handleKey={props.handleKey} />
			<br />
			<For each={props.perf_data()?.results}>
				{(
					result: {
						branch: string;
						testbed: string;
						benchmark: string;
					},
					index,
				) => (
					<>
						{index() !== 0 && <hr class="is-primary" />}
						<div class="content">
							<KeyResource
								icon="fas fa-code-branch"
								name={props.branches()?.[result.branch]?.name}
							/>
							<KeyResource
								icon="fas fa-server"
								name={props.testbeds()?.[result.testbed]?.name}
							/>
							<KeyResource
								icon="fas fa-tachometer-alt"
								name={props.benchmarks()?.[result.benchmark]?.name}
							/>
						</div>
						<KeyButton
							index={index}
							perf_active={props.perf_active}
							handlePerfActive={props.handlePerfActive}
						/>
					</>
				)}
			</For>
			<br />
			<MinimizeKeyButton handleKey={props.handleKey} />
		</>
	);
};

const MinimizedKey = (props) => {
	return (
		<>
			<MaximizeKeyButton handleKey={props.handleKey} />
			<br />
			<For each={props.perf_data()?.results}>
				{(_result, index) => (
					<KeyButton
						index={index}
						perf_active={props.perf_active}
						handlePerfActive={props.handlePerfActive}
					/>
				)}
			</For>
			<br />
			<MaximizeKeyButton handleKey={props.handleKey} />
		</>
	);
};

const MinimizeKeyButton = (props) => {
	return (
		<button
			class="button is-small is-fullwidth is-primary is-inverted"
			onClick={() => props.handleKey(false)}
		>
			<span class="icon">
				<i class="fas fa-less-than" aria-hidden="true" />
			</span>
		</button>
	);
};

const MaximizeKeyButton = (props) => {
	return (
		<button
			class="button is-small is-fullwidth is-primary is-inverted"
			onClick={() => props.handleKey(true)}
		>
			<span class="icon">
				<i class="fas fa-greater-than" aria-hidden="true" />
			</span>
		</button>
	);
};

const KeyResource = (props) => {
	return (
		<div class="columns is-vcentered is-mobile">
			<div class="column is-narrow">
				<span class="icon">
					<i class={props.icon} aria-hidden="true" />
				</span>
			</div>
			<div class="column">
				<div class="columns">
					<div class="column">
						<small style="overflow-wrap:anywhere;">{props.name}</small>
					</div>
				</div>
			</div>
		</div>
	);
};

const KeyButton = (props) => {
	const color = d3.schemeTableau10[props.index() % 10];

	return (
		<button
			// On click toggle visibility of key
			// move button over to being is-outlined
			class="button is-small is-fullwidth"
			style={
				props.perf_active[props.index()]
					? `background-color:${color};`
					: `border-color:${color};color:${color};`
			}
			onClick={() => props.handlePerfActive(props.index())}
		>
			{props.index() + 1}
		</button>
	);
};

export default PlotKey;
