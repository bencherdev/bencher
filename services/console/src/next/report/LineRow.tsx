import { tailOf } from "@bencherdev/ui/tail";
import Truncate from "@bencherdev/ui/Truncate";
import { For, type JSX, Show, Suspense, createMemo, lazy } from "solid-js";
import InlineHistory from "../plot/InlineHistory";
import { changeOf, type Guard } from "../plot/format";
import { inlineOf } from "../plot/inline";
import type { AlignedSeries } from "./lines";
import { type ParameterSet, iterationTag, lineLabel, rowNumbers } from "./row";
import { rowClick } from "./rowClick";
import Tag from "../tag";

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
	points: {
		x: readonly number[];
		report: readonly number[];
		iteration?: readonly number[];
	};
	iteration?: number;
	iterations?: number;
	chosen?: number;
}

/** The columns of a wide row; a narrow one folds them into four, and a stacked one, its numbers under its name, into three. */
export const COLUMNS = { wide: 9, narrow: 4, stacked: 3 } as const;

/** Loads once the page goes idle or the reader moves toward the lines, so no first paint waits on it. */
export const RowExplore = lazy(() => import("./RowExplore"));

interface LineRowProps {
	line: RowLine;
	/** Names the metric, for a report with more than one metric name. */
	metrics: boolean;
	narrow: boolean;
	/** Narrow, with the numbers on a line of their own under the measure. */
	stacked?: boolean;
	/** Its position among the table's rows, counting from 1. */
	index: number;
	/** Without it the row has no checkbox. */
	onSelect?: ((selected: boolean) => void) | undefined;
	selected?: boolean;
	expanded: boolean;
	onExpand: (expanded: boolean) => void;
	onNoThreshold?: (() => void) | undefined;
	/** Hovering or focusing the expand control, before the click. */
	onIntent?: () => void;
	/** What the row shows in the measure's place, such as the report an alert came from. */
	aside?: JSX.Element;
	/** What the row shows beside the measure, in a column of its own. */
	beside?: JSX.Element;
	/** No longer alerting, as a dismissed alert: drawn in the muted color, never faded. */
	dimmed?: boolean;
	/** Draws the row's own controls: a last cell when wide, after the Explore link, and a third line when narrow. */
	actions?: (() => JSX.Element) | undefined;
	/** The line's plot in Explore, once the page can write its link. */
	explore?: string | undefined;
	/** What the row's controls call its line: short, among the lines of its table. */
	name?: string | undefined;
	/** The page of the alert the row names, which its name links to. */
	href?: string | undefined;
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
		);
	const short = () => props.name ?? label().name;
	const region = () => `lr-ex-${props.line.key}`;
	const alerting = () => props.line.alert !== undefined && !props.dimmed;
	const columns = () =>
		props.narrow
			? props.stacked
				? COLUMNS.stacked
				: COLUMNS.narrow
			: COLUMNS.wide +
				(props.actions === undefined ? 0 : 1) +
				(props.beside === undefined ? 0 : 1);

	const select = () => (
		<Show when={props.onSelect}>
			{(onSelect) => (
				<label class="lr-hit">
					<input
						type="checkbox"
						class="lr-cb"
						checked={props.selected === true}
						aria-label={`Select ${short()}`}
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
			aria-label={`${props.expanded ? "Collapse" : "Expand"} ${short()}`}
			onClick={() => props.onExpand(!props.expanded)}
			onPointerEnter={() => props.onIntent?.()}
			onFocus={() => props.onIntent?.()}
		>
			<svg viewBox="0 0 24 24" aria-hidden="true">
				<path d="M9 5l7 7-7 7" />
			</svg>
		</button>
	);
	// The spaces read the name as words; between grid items they take no room.
	const words = () => (
		<>
			<b class="lr-name">
				<Truncate text={props.line.benchmark.name} tail />
				{/* Unseen, it holds the name at least as wide as its tail. */}
				<span class="lr-hold" aria-hidden="true">
					{tailOf(props.line.benchmark.name)[1]}
				</span>
			</b>
			<Show when={label().tags.length > 0 || label().metric}>
				{" "}
				{/* One track for every tag, so many long ones never starve the name. */}
				<span class="lr-tags">
					<For each={label().tags}>
						{(tag) => (
							<>
								<Tag tag={tag} />{" "}
							</>
						)}
					</For>
					<Show when={label().metric}>
						{(metric) => (
							<span class="lr-tag">
								<Truncate text={metric()} />
							</span>
						)}
					</Show>
				</span>
			</Show>
		</>
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
			<Show when={props.href} fallback={words()}>
				{(href) => (
					<a
						class="al-link"
						href={href()}
						aria-label={`${label().name}, open the alert`}
					>
						{words()}
					</a>
				)}
			</Show>
			<Show when={iterationTag(props.line)}>
				{(tag) => (
					<>
						{" "}
						<span class="lr-tag lr-fixed">{tag()}</span>
					</>
				)}
			</Show>
		</>
	);
	const inline = createMemo(() =>
		inlineOf(props.line.points, props.line.history, props.line.chosen),
	);
	const history = () => (
		<InlineHistory
			x={inline().x}
			line={inline().line}
			muted={props.dimmed === true}
			label={`${short()}, history`}
		/>
	);
	// A stacked row leaves the delta's word to its arrow and color, and to screen readers.
	const delta = (terse = false) => (
		<Show
			when={
				props.line.guard &&
				props.line.baseline != null &&
				changeOf(props.line.value, props.line.baseline, props.line.guard)
			}
		>
			{(delta) => (
				<span class={delta().tone ? `lr-${delta().tone}` : undefined}>
					<Show when={delta().arrow}>
						<span aria-hidden="true">{delta().arrow} </span>
					</Show>
					{delta().text}
					<Show when={delta().tone}>
						<span classList={{ "sr-only": terse }}> {delta().tone}</span>
					</Show>
				</span>
			)}
		</Show>
	);
	const go = () => (
		<Show when={props.explore}>
			{(href) => (
				<Suspense>
					<RowExplore href={href()} name={short()} />
				</Suspense>
			)}
		</Show>
	);
	// The value, its delta, and the limit, one under the other, or along one line when stacked.
	const narrowNumbers = () => (
		<>
			<b class="lr-num ui-num">{numbers().value}</b>
			<span class="lr-num">{delta(props.stacked === true)}</span>
			<Show when={props.line.guard} fallback={noThreshold()}>
				<span class="lr-num ui-num muted">
					<span class="sr-only">limit </span>
					{numbers().limit}
				</span>
			</Show>
		</>
	);
	const toggle = (event: MouseEvent) => {
		if (rowClick(event)) {
			props.onExpand(!props.expanded);
		}
	};
	const noThreshold = () => (
		<Show
			when={props.onNoThreshold}
			fallback={<span class="lr-quiet">no threshold</span>}
		>
			{(open) => (
				<button
					type="button"
					class="lnk lr-quiet"
					aria-label={`No threshold checks ${short()}. Show the run snippet that declares one.`}
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
						onClick={toggle}
					>
						<td>{select()}</td>
						<td>{twist()}</td>
						<td>
							<div class="lr-line">{name()}</div>
						</td>
						<td>
							{props.aside ?? <Truncate text={props.line.measure.name} />}
						</td>
						<Show when={props.beside}>{(beside) => <td>{beside()}</td>}</Show>
						<td>{history()}</td>
						<td class="lr-end lr-num ui-num">{numbers().value}</td>
						<td class="lr-num">{delta()}</td>
						<td class="lr-end ui-num">
							<Show when={props.line.guard} fallback={noThreshold()}>
								<span class="lr-num">{numbers().limit}</span>
							</Show>
						</td>
						<td class="lr-gocell">{go()}</td>
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
					onClick={toggle}
				>
					<td>{select()}</td>
					<td>
						<div class="lrn-top">
							<div class="lrn-lines">
								<div class="lrn-l1">{name()}</div>
								<div class="lrn-l2">
									<Show when={props.beside}>
										{(beside) => (
											<>
												{beside()}
												<span aria-hidden="true">·</span>
											</>
										)}
									</Show>
									<span class="lrn-measure">
										{props.aside ?? props.line.measure.name}
									</span>
									{history()}
								</div>
							</div>
							{go()}
						</div>
						<Show when={props.stacked}>
							<div class="lrn-num lrn-l3">{narrowNumbers()}</div>
						</Show>
						<Show when={props.actions}>
							{(actions) => <div class="lrn-l2 lr-act">{actions()()}</div>}
						</Show>
					</td>
					<Show when={!props.stacked}>
						<td>
							<div class="lrn-num">{narrowNumbers()}</div>
						</td>
					</Show>
					<td>{twist()}</td>
				</tr>
			</Show>
			<Show when={props.expanded}>
				<tr class="lr-exrow" aria-rowindex={props.index + 1}>
					<td colSpan={columns()}>
						<WholeName line={props.line} metrics={props.metrics} />
						<section id={region()} aria-label={`${short()}, full plot`}>
							{props.children}
						</section>
					</td>
				</tr>
			</Show>
		</>
	);
};

export default LineRow;

/** A line's whole name, at the top of its expansion, wrapped to as many lines as it takes. */
export const WholeName = (props: { line: RowLine; metrics: boolean }) => {
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
	return (
		<div class="lr-id">
			<b>{props.line.benchmark.name}</b>
			<For each={label().tags}>
				{(tag) => (
					<>
						{" "}
						<span class="lr-ptag">{tag}</span>
					</>
				)}
			</For>{" "}
			<span class="lr-tag">{props.line.measure.name}</span>
			<Show when={label().metric}>
				{(metric) => (
					<>
						{" "}
						<span class="lr-tag">{metric()}</span>
					</>
				)}
			</Show>
			<Show when={iterationTag(props.line)}>
				{(tag) => (
					<>
						{" "}
						<span class="lr-tag">{tag()}</span>
					</>
				)}
			</Show>
		</div>
	);
};
