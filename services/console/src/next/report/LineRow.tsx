import { For, type JSX, Show } from "solid-js";
import InlineHistory from "../plot/InlineHistory";
import type { Guard } from "../plot/format";
import { formatValue } from "../plot/format";
import { unitScale } from "../plot/units";
import { deltaOf } from "./delta";
import type { AlignedSeries } from "./lines";
import { type ParameterSet, lineLabel, rowNumbers } from "./row";

/** What a row draws of a line, as a report, an alert, or a threshold's page holds it. */
interface RowLine {
	key: string;
	benchmark: { name: string };
	parameters: ParameterSet;
	measure: { name: string; units: string };
	metric: string;
	value: number;
	baseline: number | undefined;
	lower_limit: number | undefined;
	upper_limit: number | undefined;
	alert: { limit: "lower" | "upper" } | undefined;
	guard: Guard | undefined;
	history: AlignedSeries;
	points: { x: readonly number[] };
}

const UNITS = { unitScale, formatValue };

/** The columns of a wide row; a narrow one folds them into four. */
export const COLUMNS = { wide: 8, narrow: 4 } as const;

interface LineRowProps {
	line: RowLine;
	/** Names the metric, for a report with more than one metric name. */
	metrics: boolean;
	narrow: boolean;
	/** Its position among the table's rows, counting from 1. */
	index: number;
	/** Without it the row has no checkbox. */
	onSelect?: ((selected: boolean) => void) | undefined;
	selected?: boolean;
	expanded: boolean;
	onExpand: (expanded: boolean) => void;
	onNoThreshold?: () => void;
	/** Hovering or focusing the expand control, before the click. */
	onIntent?: () => void;
	/** What the row shows in the measure's place, such as the report an alert came from. */
	aside?: JSX.Element;
	/** No longer alerting, as a dismissed alert: drawn in the muted color, never faded. */
	dimmed?: boolean;
	/** Draws the row's own controls: a last cell when wide, a third line when narrow. */
	actions?: (() => JSX.Element) | undefined;
	/** What the row expands into, drawn in a row of its own under it. */
	children?: JSX.Element;
}

/** One line: the line, its measure, its history inline, the value, the delta, and the limit. */
const LineRow = (props: LineRowProps) => {
	const label = () =>
		lineLabel(
			{
				benchmark: props.line.benchmark.name,
				parameters: props.line.parameters,
				measure: props.line.measure.name,
				metric: props.line.metric,
			},
			props.metrics,
		);
	const numbers = () =>
		rowNumbers(
			{
				value: props.line.value,
				baseline: props.line.baseline,
				lower_limit: props.line.lower_limit,
				upper_limit: props.line.upper_limit,
				alert: props.line.alert,
			},
			props.line.guard,
			props.line.measure.units,
			UNITS,
		);
	const region = () => `lr-ex-${props.line.key}`;
	const alerting = () => props.line.alert !== undefined && !props.dimmed;
	const columns = () =>
		(props.narrow ? COLUMNS.narrow : COLUMNS.wide) +
		(props.actions === undefined || props.narrow ? 0 : 1);

	const select = () => (
		<Show when={props.onSelect}>
			{(onSelect) => (
				<label class="lr-hit">
					<input
						type="checkbox"
						class="lr-cb"
						checked={props.selected === true}
						aria-label={`Select ${label().name}`}
						onChange={(event) => onSelect()(event.currentTarget.checked)}
					/>
				</label>
			)}
		</Show>
	);
	const twist = () => (
		<button
			type="button"
			class="lr-twist"
			aria-expanded={props.expanded}
			aria-controls={region()}
			aria-label={`${props.expanded ? "Collapse" : "Expand"} ${label().name}`}
			onClick={() => props.onExpand(!props.expanded)}
			onPointerEnter={() => props.onIntent?.()}
			onFocus={() => props.onIntent?.()}
		>
			<svg viewBox="0 0 24 24" aria-hidden="true">
				<path d="M9 5l7 7-7 7" />
			</svg>
		</button>
	);
	const name = () => (
		<>
			<Show when={alerting()}>
				<svg
					class="lr-alertdot"
					viewBox="0 0 8 8"
					role="img"
					aria-label="alerting"
				>
					<circle cx="4" cy="4" r="4" />
				</svg>
			</Show>
			<b>{props.line.benchmark.name}</b>
			<For each={label().tags}>
				{(tag) => <span class="lr-ptag">{tag}</span>}
			</For>
			<Show when={label().metric}>
				{(metric) => <span class="lr-tag">{metric()}</span>}
			</Show>
		</>
	);
	const history = () => (
		<InlineHistory
			x={props.line.points.x}
			line={props.line.history}
			muted={props.dimmed === true}
			label={`${label().name}, history`}
		/>
	);
	const delta = () => (
		<Show
			when={deltaOf(props.line.value, props.line.baseline, props.line.guard)}
		>
			{(delta) => (
				<span class={delta().word ? `lr-${delta().word}` : undefined}>
					<Show when={delta().arrow}>
						<span aria-hidden="true">{delta().arrow} </span>
					</Show>
					{delta().text}
					<Show when={delta().word}> {delta().word}</Show>
				</span>
			)}
		</Show>
	);
	const noThreshold = () => (
		<Show when={props.onNoThreshold}>
			{(open) => (
				<button
					type="button"
					class="lnk lr-quiet"
					aria-label={`No threshold checks ${label().name}. Show the run snippet that declares one.`}
					onClick={() => open()()}
				>
					no threshold
				</button>
			)}
		</Show>
	);

	return (
		<>
			<Show
				when={props.narrow}
				fallback={
					<tr
						class="lr"
						classList={{ "lr-alerting": alerting(), "lr-dim": props.dimmed }}
						aria-rowindex={props.index}
					>
						<td>{select()}</td>
						<td>{twist()}</td>
						<td>
							<div class="lr-line" title={label().name}>
								{name()}
							</div>
						</td>
						<td>{props.aside ?? props.line.measure.name}</td>
						<td>{history()}</td>
						<td class="lr-end lr-num">{numbers().value}</td>
						<td class="lr-num">{delta()}</td>
						<td class="lr-end">
							<Show when={props.line.guard} fallback={noThreshold()}>
								<span class="lr-num">{numbers().limit}</span>
							</Show>
						</td>
						<Show when={props.actions}>
							{(actions) => <td class="lr-act">{actions()()}</td>}
						</Show>
					</tr>
				}
			>
				<tr
					class="lrn"
					classList={{
						"lr-alerting": alerting(),
						"lr-dim": props.dimmed,
						"lrn-act": props.actions !== undefined,
					}}
					aria-rowindex={props.index}
				>
					<td>{select()}</td>
					<td title={label().name}>
						<div class="lrn-l1">{name()}</div>
						<div class="lrn-l2">
							<span>{props.aside ?? props.line.measure.name}</span>
							{history()}
							<Show when={props.line.guard} fallback={noThreshold()}>
								<span class="lr-num muted">limit {numbers().limit}</span>
							</Show>
						</div>
						<Show when={props.actions}>
							{(actions) => <div class="lrn-l2 lr-act">{actions()()}</div>}
						</Show>
					</td>
					<td>
						<div class="lrn-num">
							<b class="lr-num">{numbers().value}</b>
							<span class="lr-num">{delta()}</span>
						</div>
					</td>
					<td>{twist()}</td>
				</tr>
			</Show>
			<Show when={props.expanded}>
				<tr class="lr-exrow" aria-rowindex={props.index + 1}>
					<td colSpan={columns()}>
						<section id={region()} aria-label={`${label().name}, full plot`}>
							{props.children}
						</section>
					</td>
				</tr>
			</Show>
		</>
	);
};

export default LineRow;
