import * as d3 from "d3";
import { type Accessor, For, type Resource, Show, createMemo } from "solid-js";
import { BENCHMARK_ICON } from "../../../../../config/project/benchmarks";
import { BRANCH_ICON } from "../../../../../config/project/branches";
import { MEASURE_ICON } from "../../../../../config/project/measures";
import { TESTBED_ICON } from "../../../../../config/project/testbeds";
import type { JsonPerf, JsonPerfMetrics } from "../../../../../types/bencher";

export interface Props {
	key: Accessor<boolean>;
	handleKey: (key: boolean) => void;
	perfData: Resource<JsonPerf>;
	perfActive: boolean[];
	handlePerfActive: (index: number) => void;
	togglePerfActive: () => void;
}

const PlotKey = (props: Props) => {
	return (
		<Show
			when={props.key()}
			fallback={
				<MinimizedKey
					perfData={props.perfData}
					perfActive={props.perfActive}
					handleKey={props.handleKey}
					handlePerfActive={props.handlePerfActive}
					togglePerfActive={props.togglePerfActive}
				/>
			}
		>
			<ExpandedKey
				perfData={props.perfData}
				perfActive={props.perfActive}
				handleKey={props.handleKey}
				handlePerfActive={props.handlePerfActive}
				togglePerfActive={props.togglePerfActive}
			/>
		</Show>
	);
};

const ExpandedKey = (props: {
	perfData: Resource<JsonPerf>;
	handleKey: (key: boolean) => void;
	perfActive: boolean[];
	handlePerfActive: (index: number) => void;
	togglePerfActive: () => void;
}) => {
	const dimensions = (resultDimension: (result: JsonPerfMetrics) => string) =>
		props.perfData()?.results?.reduce((set, result) => {
			return set.add(resultDimension(result));
		}, new Set());
	const branches = createMemo(() =>
		dimensions((result) => result.branch?.name),
	);
	const testbeds = createMemo(() =>
		dimensions((result) => result.testbed?.name),
	);
	const benchmarks = createMemo(() =>
		dimensions((result) => result.benchmark?.name),
	);
	const measures = createMemo(() =>
		dimensions((result) => result.measure?.name),
	);

	return (
		<div class="columns is-centered is-gapless is-multiline">
			<div class="column is-narrow">
				<MinimizeKeyButton handleKey={props.handleKey} />
				<br />
				<KeyToggle
					perfActive={props.perfActive}
					togglePerfActive={props.togglePerfActive}
				/>
			</div>
			<Show
				when={
					(props.perfData()?.results?.length ?? 0) > 1 &&
					(branches()?.size === 1 ||
						testbeds()?.size === 1 ||
						benchmarks()?.size === 1 ||
						measures()?.size === 1)
				}
			>
				<div class="column is-3">
					<div class="columns is-centered">
						<div class="column is-11">
							<div class="box">
								<Show when={branches()?.size === 1}>
									<KeyResource
										icon={BRANCH_ICON}
										name={branches()?.values().next().value}
									/>
								</Show>
								<Show when={testbeds()?.size === 1}>
									<KeyResource
										icon={TESTBED_ICON}
										name={testbeds()?.values().next().value}
									/>
								</Show>
								<Show when={benchmarks()?.size === 1}>
									<KeyResource
										icon={BENCHMARK_ICON}
										name={benchmarks()?.values().next().value}
									/>
								</Show>
								<Show when={measures()?.size === 1}>
									<KeyResource
										icon={MEASURE_ICON}
										name={measures()?.values().next().value}
									/>
								</Show>
							</div>
						</div>
					</div>
				</div>
			</Show>
			<For each={props.perfData()?.results}>
				{(
					result: {
						branch: { name: string };
						testbed: { name: string };
						benchmark: { name: string };
						measure: { name: string };
					},
					index,
				) => (
					<div class="column is-2">
						<KeyButton
							index={index}
							perfActive={props.perfActive}
							handlePerfActive={props.handlePerfActive}
						/>
						<Show
							when={
								props.perfData()?.results?.length === 1 ||
								branches()?.size !== 1
							}
						>
							<KeyResource icon={BRANCH_ICON} name={result.branch?.name} />
						</Show>
						<Show
							when={
								props.perfData()?.results?.length === 1 ||
								testbeds()?.size !== 1
							}
						>
							<KeyResource icon={TESTBED_ICON} name={result.testbed?.name} />
						</Show>
						<Show
							when={
								props.perfData()?.results?.length === 1 ||
								benchmarks()?.size !== 1
							}
						>
							<KeyResource
								icon={BENCHMARK_ICON}
								name={result.benchmark?.name}
							/>
						</Show>
						<Show
							when={
								props.perfData()?.results?.length === 1 ||
								measures()?.size !== 1
							}
						>
							<KeyResource icon={MEASURE_ICON} name={result.measure?.name} />
						</Show>
					</div>
				)}
			</For>
		</div>
	);
};

const MinimizedKey = (props: {
	perfData: Resource<JsonPerf>;
	handleKey: (key: boolean) => void;
	perfActive: boolean[];
	handlePerfActive: (index: number) => void;
	togglePerfActive: () => void;
}) => {
	return (
		<div class="columns is-centered is-vcentered is-gapless is-multiline is-mobile">
			<div class="column is-narrow">
				<MaximizeKeyButton handleKey={props.handleKey} />
			</div>
			<div class="column is-narrow">
				<KeyToggle
					perfActive={props.perfActive}
					togglePerfActive={props.togglePerfActive}
				/>
			</div>
			<For each={props.perfData()?.results}>
				{(_result, index) => (
					<div class="column is-narrow">
						<KeyButton
							index={index}
							perfActive={props.perfActive}
							handlePerfActive={props.handlePerfActive}
						/>
					</div>
				)}
			</For>
		</div>
	);
};

const MinimizeKeyButton = (props: { handleKey: (key: boolean) => void }) => {
	return (
		<button
			title="Minimize Key"
			type="button"
			class="button is-small is-fullwidth"
			onMouseDown={() => props.handleKey(false)}
		>
			<span class="icon has-text-primary">
				<i class="far fa-minus-square fa-2x" />
			</span>
		</button>
	);
};

const MaximizeKeyButton = (props: { handleKey: (key: boolean) => void }) => {
	return (
		<button
			title="Expand Key"
			type="button"
			class="button is-small is-fullwidth"
			onMouseDown={() => props.handleKey(true)}
		>
			<span class="icon has-text-primary">
				<i class="far fa-plus-square fa-2x" />
			</span>
		</button>
	);
};

const KeyResource = (props: { icon: string; name: string }) => {
	return (
		<div>
			<span class="icon">
				<i class={props.icon} />
			</span>
			<small style="word-break: break-all;">
				<Show when={props.name} fallback={"Loading..."}>
					{props.name}
				</Show>
			</small>
		</div>
	);
};

const KeyButton = (props: {
	index: Accessor<number>;
	perfActive: boolean[];
	handlePerfActive: (index: number) => void;
}) => {
	const color = d3.schemeTableau10[props.index() % 10];
	const number = props.index() + 1;
	return (
		<button
			// On click toggle visibility of key
			class="button is-small is-fullwidth"
			type="button"
			title={
				props.perfActive[props.index()]
					? `Hide Plot ${number}`
					: `Show Plot ${number}`
			}
			style={
				props.perfActive[props.index()]
					? `background-color:${color};`
					: `border-color:${color};color:${color};`
			}
			onMouseDown={() => props.handlePerfActive(props.index())}
		>
			{number}
		</button>
	);
};

const KeyToggle = (props: {
	perfActive: boolean[];
	togglePerfActive: () => void;
}) => {
	const allActive = createMemo(() =>
		props.perfActive.reduce((acc, curr) => {
			return acc && curr;
		}, true),
	);

	return (
		<button
			class="button is-small is-fullwidth"
			type="button"
			title={allActive() ? "Hide all plots" : "Show all plots"}
			onMouseDown={() => {
				props.togglePerfActive();
			}}
		>
			<span class="icon has-text-primary">
				<Show when={allActive()} fallback={<i class="far fa-eye fa-1x" />}>
					<i class="far fa-eye-slash fa-1x" />
				</Show>
			</span>
		</button>
	);
};

export default PlotKey;
