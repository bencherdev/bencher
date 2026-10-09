import Button from "@bencherdev/ui/Button";
import Table from "@bencherdev/ui/Table";
import {
	For,
	Show,
	createEffect,
	createMemo,
	createSignal,
	on,
	onCleanup,
	onMount,
} from "solid-js";
import { projectPath } from "../paths";
import { useNarrow } from "../plot/narrow";
import { type Range, visibleRange } from "../reports/rows";
import {
	archivedBy,
	dayText,
	filterSets,
	filterSummary,
	modelText,
} from "./model";
import { ROW_HEIGHT } from "./query";
import Sets from "./Sets";
import type { ThresholdRow } from "./rows";

/** Rows drawn above and below the screen, so a quick scroll never shows a gap. */
const OVERSCAN = 8;
const COLUMNS = 7;

/**
 * The thresholds, oldest first, drawing only the rows on screen: rows above
 * and below them are spacers of the same height.
 */
const ThresholdsTable = (props: {
	slug: string;
	rows: ThresholdRow[];
	/** Every threshold the search matches, loaded or not. */
	total: number;
	archived: boolean;
	/** The window as the table's name says it, "in 4 weeks". */
	window: string;
	/** The window as the alerts column's header says it, "4w". */
	windowLabel: string;
	/** The rows on screen changed, not counting those drawn either side. */
	onRange?: (range: Range) => void;
	/** The rows are a search's before the last, while the new one loads. */
	busy?: boolean;
	loading?: boolean;
	/** The next batch failed; asks for it again. */
	onRetry?: (() => void) | undefined;
}) => {
	const narrow = useNarrow();
	const [range, setRange] = createSignal<Range>({ start: 0, end: 0 });
	let body: HTMLTableSectionElement | undefined;
	let onScreen: Range = { start: 0, end: 0 };
	const rowHeight = () => (narrow() ? ROW_HEIGHT.narrow : ROW_HEIGHT.wide);

	const measure = () => {
		if (!body) {
			return;
		}
		const place = {
			top: body.getBoundingClientRect().top,
			viewport: window.innerHeight,
			rowHeight: rowHeight(),
			count: props.rows.length,
		};
		const next = visibleRange({ ...place, overscan: OVERSCAN });
		const current = range();
		if (next.start !== current.start || next.end !== current.end) {
			setRange(next);
		}
		const visible = visibleRange({ ...place, overscan: 0 });
		if (visible.start !== onScreen.start || visible.end !== onScreen.end) {
			onScreen = visible;
			props.onRange?.(visible);
		}
	};
	let frame: number | undefined;
	const schedule = () => {
		frame ??= requestAnimationFrame(() => {
			frame = undefined;
			measure();
		});
	};
	onMount(() => {
		window.addEventListener("scroll", schedule, { passive: true });
		window.addEventListener("resize", schedule, { passive: true });
		onCleanup(() => {
			window.removeEventListener("scroll", schedule);
			window.removeEventListener("resize", schedule);
			if (frame !== undefined) {
				cancelAnimationFrame(frame);
			}
		});
	});
	createEffect(on([() => props.rows.length, narrow], measure));

	const shown = createMemo(() => props.rows.slice(range().start, range().end));
	const above = () => range().start * rowHeight();
	const below = () => (props.rows.length - range().end) * rowHeight();

	return (
		<Table
			class="th-table"
			aria-label={`${props.archived ? "Archived thresholds" : "Thresholds"}, with the alerts each raised ${props.window}`}
			aria-rowcount={props.total + 1}
			aria-busy={props.busy === true ? "true" : undefined}
			fold
		>
			<thead>
				<tr aria-rowindex={1}>
					<th scope="col">Branch</th>
					<th scope="col">Testbed</th>
					<th scope="col">Measure</th>
					<th scope="col">Metric</th>
					<th scope="col">Parameters</th>
					<th scope="col">Model</th>
					<th scope="col">Alerts in {props.windowLabel}</th>
				</tr>
			</thead>
			<tbody ref={body}>
				<Show when={above() > 0}>
					<Spacer height={above()} />
				</Show>
				<For each={shown()}>
					{(row, index) => (
						<Row
							slug={props.slug}
							row={row}
							narrow={narrow()}
							index={range().start + index()}
						/>
					)}
				</For>
				<Show when={below() > 0}>
					<Spacer height={below()} />
				</Show>
				<Show when={props.loading}>
					{/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: a placeholder row has nothing to focus or read */}
					<tr class="th-spacer th-more" aria-hidden="true">
						<td colSpan={COLUMNS}>
							<span class="ui-skeleton" data-size="text" />
						</td>
					</tr>
				</Show>
				<Show when={!props.loading && props.onRetry}>
					{(retry) => (
						<tr class="th-more">
							<td colSpan={COLUMNS}>
								<div class="reports-failed" role="alert">
									<span>
										These thresholds did not load: the Bencher API did not
										answer.
									</span>
									<Button size="sm" onClick={() => retry()()}>
										Retry
									</Button>
								</div>
							</td>
						</tr>
					)}
				</Show>
			</tbody>
		</Table>
	);
};

export default ThresholdsTable;

const Spacer = (props: { height: number }) => (
	// biome-ignore lint/a11y/noAriaHiddenOnFocusable: a spacer row only holds the height of the rows not drawn
	<tr
		class="th-spacer"
		aria-hidden="true"
		style={{ height: `${props.height}px` }}
	>
		<td colSpan={COLUMNS} />
	</tr>
);

/** The threshold's link names it by what it applies to. */
const thresholdName = (row: ThresholdRow) =>
	`Threshold on ${row.branch.name}, ${row.testbed.name}, ${row.measure.name}, ${row.metric}, ${
		row.parameters?.length
			? `filtered to ${filterSummary(row.parameters)}`
			: "every variant"
	}`;

const Archived = (props: { at: number | undefined }) => (
	<Show when={props.at}>
		{(at) => (
			<span class="th-from"> · archived {dayText(at(), Date.now())}</span>
		)}
	</Show>
);

const Row = (props: {
	slug: string;
	row: ThresholdRow;
	narrow: boolean;
	index: number;
}) => {
	const model = () => modelText(props.row.model);
	return (
		<tr class="th-row" aria-rowindex={props.index + 2}>
			<td data-fold="l2" class="th-branch">
				{/* On a phone this line's end is cut off, so what archived the row comes first. */}
				<Show when={props.narrow && archivedBy(props.row)}>
					{(by) => (
						<span class="th-from">
							{by().kind} archived {dayText(by().archived, Date.now())} ·{" "}
						</span>
					)}
				</Show>
				<a
					class="th-link"
					href={`${projectPath(props.slug, "thresholds")}/${props.row.uuid}`}
					aria-label={thresholdName(props.row)}
				>
					{props.narrow
						? [
								props.row.branch.name,
								props.row.testbed.name,
								filterSummary(props.row.parameters),
								props.row.model?.test ?? "no model",
							].join(" · ")
						: props.row.branch.name}
				</a>
				<Show when={!props.narrow && props.row.branch.start_point}>
					{(from) => <span class="th-from"> from {from()}</span>}
				</Show>
				<Show when={!props.narrow}>
					<Archived at={props.row.branch.archived} />
				</Show>
			</td>
			<td data-fold="hide">
				{props.row.testbed.name}
				<Archived at={props.row.testbed.archived} />
			</td>
			<td data-fold="l1">
				{props.narrow
					? `${props.row.measure.name} · ${props.row.metric}`
					: props.row.measure.name}
				<Show when={!props.narrow}>
					<Archived at={props.row.measure.archived} />
				</Show>
			</td>
			<td data-fold="hide" class="th-mono">
				{props.row.metric}
			</td>
			<td data-fold="hide">
				<Show
					when={props.row.parameters?.length}
					fallback={<span class="th-quiet">every variant</span>}
				>
					<Sets sets={filterSets(props.row.parameters)} />
				</Show>
			</td>
			<td data-fold="hide" class="th-model th-mono" title={model()}>
				{model()}
			</td>
			<td data-fold="n1">
				<span class="th-counts">
					<Show when={props.row.active > 0}>
						<span class="th-active">
							<span class="th-dot" aria-hidden="true" />
							{props.row.active} active
						</span>{" "}
					</Show>
					<span class="th-quiet">{props.row.raised} raised</span>
				</span>
			</td>
		</tr>
	);
};
