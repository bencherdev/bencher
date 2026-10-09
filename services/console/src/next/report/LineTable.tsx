import Button from "@bencherdev/ui/Button";
import {
	type Component,
	For,
	type JSX,
	Show,
	Suspense,
	createEffect,
	createMemo,
	createSignal,
	lazy,
	on,
	onCleanup,
	onMount,
	untrack,
} from "solid-js";
import type { PlotData } from "../plot/types";
import type { GroupRow, ReportLine } from "./lines";
import LineRow, { COLUMNS } from "./LineRow";
import { type Slot, isGroup, offsetsOf, slotRange } from "./slots";

const RowPlot = lazy(() => import("./RowPlot"));

/** The fixed heights the list lays out by, wide and narrow; report.css draws the same. */
export const HEIGHTS = {
	wide: { group: 38, line: 44, plot: 330, actions: 0 },
	narrow: { group: 44, line: 64, plot: 360, actions: 20 },
} as const;

/** Pixels drawn above and below the screen, so a quick scroll never shows a gap. */
const OVERSCAN = 400;

interface LineTableProps {
	label: string;
	slots: readonly Slot[];
	/** Every report line and group header, loaded or not. */
	rows: number;
	narrow: boolean;
	metrics: boolean;
	history: string;
	expanded: ReadonlySet<string>;
	onExpand: (line: ReportLine, expanded: boolean) => void;
	/** Without it the rows have no checkboxes. */
	selected?: ReadonlySet<string> | undefined;
	onSelect?: (line: ReportLine, selected: boolean) => void;
	onNoThreshold?: (line: ReportLine) => void;
	/** A column in the measure's place: its header, its width's class, and each row's cell. */
	aside?: {
		label: string;
		column: string;
		cell: (line: ReportLine) => JSX.Element;
	};
	/** A page's own header for each group, drawn at its own heights. */
	group?: {
		height: { wide: number; narrow: number };
		row: Component<{ group: GroupRow; index: number; columns: number }>;
	};
	/** Each line's own controls: a last column when wide, and when narrow a third line for the lines `narrow` names. */
	actions?: {
		cell: (line: ReportLine) => JSX.Element;
		narrow: (line: ReportLine) => boolean;
	};
	/** The lines drawn dimmed. */
	dimmed?: (line: ReportLine) => boolean;
	plot: (line: ReportLine) => {
		data: PlotData;
		note: string;
		reportHref: (uuid: string) => string;
	};
	/** The rows drawn below the screen reach the last one loaded. */
	onNearEnd?: () => void;
	/** The rows are a view's before the last, while the new one loads. */
	busy?: boolean;
	loading?: boolean;
	/** The next batch failed; asks for it again. */
	onRetry?: (() => void) | undefined;
}

/**
 * A report's lines under their group headers, drawing only the rows on screen:
 * the rows above and below them are spacers of the height they would take.
 */
const LineTable = (props: LineTableProps) => {
	const heights = () => (props.narrow ? HEIGHTS.narrow : HEIGHTS.wide);
	const columns = () =>
		props.narrow
			? COLUMNS.narrow
			: COLUMNS.wide + (props.actions === undefined ? 0 : 1);
	const open = (slot: Slot) => !isGroup(slot) && props.expanded.has(slot.key);
	/** Whether a line draws its controls: always in their column when wide, on a line of their own when narrow. */
	const acts = (line: ReportLine) =>
		props.actions !== undefined &&
		(!props.narrow || props.actions.narrow(line));
	const offsets = createMemo(() =>
		offsetsOf(
			props.slots.map((slot) =>
				isGroup(slot)
					? (props.group?.height[props.narrow ? "narrow" : "wide"] ??
						heights().group)
					: heights().line +
						(acts(slot) ? heights().actions : 0) +
						(open(slot) ? heights().plot : 0),
			),
		),
	);
	/** Each slot's row index: an expanded line takes two rows, the header one. */
	const indexes = createMemo(() => {
		let index = props.narrow ? 1 : 2;
		return props.slots.map((slot) => {
			const current = index;
			index += open(slot) ? 2 : 1;
			return current;
		});
	});
	const opened = () => props.slots.filter(open).length;
	const [range, setRange] = createSignal({ start: 0, end: 0 });
	let body: HTMLTableSectionElement | undefined;

	const measure = () => {
		if (!body) {
			return;
		}
		const next = slotRange({
			offsets: offsets(),
			top: body.getBoundingClientRect().top,
			viewport: window.innerHeight,
			overscan: OVERSCAN,
		});
		const current = range();
		if (next.start !== current.start || next.end !== current.end) {
			setRange(next);
		}
		if (next.end >= props.slots.length) {
			props.onNearEnd?.();
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
	createEffect(on(offsets, measure));

	const shown = createMemo(() => props.slots.slice(range().start, range().end));
	const above = () => offsets()[range().start] ?? 0;
	const below = () => (offsets().at(-1) ?? 0) - (offsets()[range().end] ?? 0);

	return (
		<table
			class="lr-table"
			classList={{ "lr-narrow": props.narrow }}
			aria-label={props.label}
			aria-rowcount={(props.narrow ? 0 : 1) + props.rows + opened()}
			aria-busy={props.busy === true ? "true" : undefined}
		>
			<Show
				when={props.narrow}
				fallback={
					<>
						<colgroup>
							<col class="lr-c-select" />
							<col class="lr-c-twist" />
							<col />
							<col class={props.aside?.column ?? "lr-c-measure"} />
							<col class="lr-c-history" />
							<col class="lr-c-value" />
							<col class="lr-c-delta" />
							<col class="lr-c-limit" />
							<Show when={props.actions}>
								<col class="lr-c-act" />
							</Show>
						</colgroup>
						<thead>
							<tr class="lr-head" aria-rowindex={1}>
								<th scope="col">
									<span class="sr-only">Select</span>
								</th>
								<th scope="col">
									<span class="sr-only">Expand</span>
								</th>
								<th scope="col">Line</th>
								<th scope="col">{props.aside?.label ?? "Measure"}</th>
								<th scope="col">{props.history}</th>
								<th scope="col" class="lr-end">
									Value
								</th>
								<th scope="col">
									<span aria-hidden="true">Δ</span>
									<span class="sr-only">Delta</span>
								</th>
								<th scope="col" class="lr-end">
									Limit
								</th>
								<Show when={props.actions}>
									<th scope="col">
										<span class="sr-only">Actions</span>
									</th>
								</Show>
							</tr>
						</thead>
					</>
				}
			>
				<colgroup>
					<col class="lr-c-hit" />
					<col />
					<col class="lr-c-numbers" />
					<col class="lr-c-hit" />
				</colgroup>
			</Show>
			<tbody ref={body}>
				<Spacer height={above()} columns={columns()} />
				<For each={shown()}>
					{(slot, position) => {
						const index = () => indexes()[range().start + position()] ?? 0;
						// Read once, so the plot keeps its data while the page around it changes.
						let plot: ReturnType<LineTableProps["plot"]> | undefined;
						const plotOf = (line: ReportLine) =>
							(plot ??= untrack(() => props.plot(line)));
						const aside = isGroup(slot)
							? undefined
							: untrack(() => props.aside?.cell(slot));
						const Header = props.group?.row ?? Group;
						return isGroup(slot) ? (
							<Header group={slot} index={index()} columns={columns()} />
						) : (
							<LineRow
								line={slot}
								metrics={props.metrics}
								narrow={props.narrow}
								index={index()}
								selected={props.selected?.has(slot.key) === true}
								onSelect={
									props.selected && props.onSelect
										? (selected) => props.onSelect?.(slot, selected)
										: undefined
								}
								expanded={props.expanded.has(slot.key)}
								onExpand={(expanded) => props.onExpand(slot, expanded)}
								onNoThreshold={() => props.onNoThreshold?.(slot)}
								aside={aside}
								dimmed={props.dimmed?.(slot) === true}
								actions={
									acts(slot) ? () => props.actions?.cell(slot) : undefined
								}
								onIntent={() => {
									RowPlot.preload();
								}}
							>
								<AfterPaint>
									<Suspense>
										<RowPlot {...plotOf(slot)} />
									</Suspense>
								</AfterPaint>
							</LineRow>
						);
					}}
				</For>
				<Spacer height={below()} columns={columns()} />
				<Show when={props.loading}>
					{/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: a placeholder row has nothing to focus or read */}
					<tr class="lr-more" aria-hidden="true">
						<td colSpan={columns()}>
							<span class="ui-skeleton" data-size="text" />
						</td>
					</tr>
				</Show>
				<Show when={!props.loading && props.onRetry}>
					{(retry) => (
						<tr class="lr-more">
							<td colSpan={columns()}>
								<div class="lr-failed" role="alert">
									<span>
										These lines did not load: the Bencher API did not answer.
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
		</table>
	);
};

export default LineTable;

const Spacer = (props: { height: number; columns: number }) => (
	<Show when={props.height > 0}>
		{/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: a spacer row only holds the height of the rows not drawn */}
		<tr
			class="lr-spacer"
			aria-hidden="true"
			style={{ height: `${props.height}px` }}
		>
			<td colSpan={props.columns} />
		</tr>
	</Show>
);

const Group = (props: { group: GroupRow; index: number; columns: number }) => (
	<tr class="lr-group" aria-rowindex={props.index}>
		<td colSpan={props.columns}>
			<b>{props.group.name}</b> <Dot />{" "}
			{props.group.variants === 1
				? "1 variant"
				: `${props.group.variants} variants`}
			, {props.group.lines === 1 ? "1 line" : `${props.group.lines} lines`}
			<Show when={props.group.alerts > 0}>
				{" "}
				<Dot />{" "}
				<span class="lr-galert">
					{props.group.alerts === 1
						? "1 alert"
						: `${props.group.alerts} alerts`}
				</span>
			</Show>
		</td>
	</tr>
);

const Dot = () => <span aria-hidden="true">·</span>;

/** Its children, once the frame that showed the space they fill has painted. */
const AfterPaint = (props: { children: JSX.Element }) => {
	const [painted, setPainted] = createSignal(false);
	onMount(() => {
		let timer: ReturnType<typeof setTimeout> | undefined;
		const frame = requestAnimationFrame(() => {
			timer = setTimeout(() => setPainted(true));
		});
		onCleanup(() => {
			cancelAnimationFrame(frame);
			clearTimeout(timer);
		});
	});
	return <Show when={painted()}>{props.children}</Show>;
};
