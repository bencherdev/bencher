import * as d3 from "d3";
import { type Accessor, For, type Resource, Show, createMemo } from "solid-js";
import type { JsonPerf } from "../../../../../types/bencher";

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
			<For each={props.perfData()?.results}>
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
							perfActive={props.perfActive}
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
			class="button is-small is-fullwidth is-primary is-inverted"
			onClick={() => props.handleKey(false)}
		>
			<span class="icon">
				<i class="far fa-minus-square fa-2x" aria-hidden="true" />
			</span>
		</button>
	);
};

const MaximizeKeyButton = (props: { handleKey: (key: boolean) => void }) => {
	return (
		<button
			title="Expand Key"
			type="button"
			class="button is-small is-fullwidth is-primary is-inverted"
			onClick={() => props.handleKey(true)}
		>
			<span class="icon">
				<i class="far fa-plus-square fa-2x" aria-hidden="true" />
			</span>
		</button>
	);
};

const KeyResource = (props: { icon: string; name: string }) => {
	return (
		<div>
			<span class="icon">
				<i class={props.icon} aria-hidden="true" />
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
			// move button over to being is-outlined
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
			onClick={() => props.handlePerfActive(props.index())}
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
			class="button is-small is-fullwidth is-primary is-inverted"
			type="button"
			title={allActive() ? `Hide all plots` : `Show all plots`}
			onClick={() => {
				props.togglePerfActive();
			}}
		>
			<span class="icon">
				<Show
					when={allActive()}
					fallback={<i class="far fa-eye fa-1x" aria-hidden="true" />}
				>
					<i class="far fa-eye-slash fa-1x" aria-hidden="true" />
				</Show>
			</span>
		</button>
	);
};

export default PlotKey;
