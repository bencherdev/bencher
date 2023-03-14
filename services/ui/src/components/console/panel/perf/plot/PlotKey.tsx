import { For, Show } from "solid-js";
import * as d3 from "d3";

const PlotKey = (props) => {
	return (
		<Show
			when={props.key()}
			fallback={
				<MinimizedKey
					perf_data={props.perf_data}
					perf_active={props.perf_active}
					handleKey={props.handleKey}
					handlePerfActive={props.handlePerfActive}
				/>
			}
		>
			<ExpandedKey
				perf_data={props.perf_data}
				perf_active={props.perf_active}
				handleKey={props.handleKey}
				handlePerfActive={props.handlePerfActive}
			/>
		</Show>
	);
};

const ExpandedKey = (props) => {
	return (
		<div class="columns is-centered is-gapless is-multiline">
			<div class="column is-narrow">
				<MinimizeKeyButton handleKey={props.handleKey} />
			</div>
			<For each={props.perf_data()?.results}>
				{(
					result: {
						branch: { name: string };
						testbed: { name: string };
						benchmark: { name: string };
					},
					index,
				) => (
					<div class="column is-2">
						<KeyButton
							index={index}
							perf_active={props.perf_active}
							handlePerfActive={props.handlePerfActive}
						/>
						<KeyResource icon="fas fa-code-branch" name={result.branch?.name} />
						<KeyResource icon="fas fa-server" name={result.testbed?.name} />
						<KeyResource
							icon="fas fa-tachometer-alt"
							name={result.benchmark?.name}
						/>
					</div>
				)}
			</For>
		</div>
	);
};

const MinimizedKey = (props) => {
	return (
		<div class="columns is-centered is-vcentered is-gapless is-multiline is-mobile">
			<div class="column is-narrow">
				<MaximizeKeyButton handleKey={props.handleKey} />
			</div>
			<For each={props.perf_data()?.results}>
				{(_result, index) => (
					<div class="column is-narrow">
						<KeyButton
							index={index}
							perf_active={props.perf_active}
							handlePerfActive={props.handlePerfActive}
						/>
					</div>
				)}
			</For>
		</div>
	);
};

const MinimizeKeyButton = (props) => {
	return (
		<button
			class="button is-small is-fullwidth is-primary is-inverted"
			onClick={() => props.handleKey(false)}
		>
			<span class="icon">
				<i class="far fa-minus-square fa-2x" aria-hidden="true" />
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
				<i class="far fa-plus-square fa-2x" aria-hidden="true" />
			</span>
		</button>
	);
};

const KeyResource = (props) => {
	return (
		<div>
			<span class="icon">
				<i class={props.icon} aria-hidden="true" />
			</span>
			<small style="overflow-wrap:anywhere;">
				<Show when={props.name} fallback={"Loading..."}>
					{props.name}
				</Show>
			</small>
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
