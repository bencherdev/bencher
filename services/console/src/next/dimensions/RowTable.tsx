import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import Table from "@bencherdev/ui/Table";
import {
	For,
	type JSX,
	Show,
	createEffect,
	createMemo,
	createSignal,
	on,
	onCleanup,
	onMount,
} from "solid-js";
import type {
	JsonConsoleBenchmarkRow,
	JsonConsoleBranchRow,
	JsonConsoleMeasureRow,
	JsonConsoleTestbedRow,
} from "../../types/bencher";
import { NEXT_PROJECTS } from "../paths";
import { useNarrow } from "../plot/narrow";
import { type Range, visibleRange } from "../reports/rows";
import { ROW_HEIGHT, type Row, thresholdsOf } from "./data";
import { type Dimension, KINDS } from "./dimension";
import { archiveImpact } from "./impact";
import { shortDate, shortDay } from "./time";

/** Rows drawn above and below the screen, so a quick scroll never shows a gap. */
const OVERSCAN = 8;

/**
 * A dimension's rows, drawing only those on screen: rows above and below them
 * are spacers of the same height. The row whose Archive is open shows its
 * impact line below it, and closes once it scrolls out of the rows drawn.
 */
const RowTable = (props: {
	slug: string;
	dimension: Dimension;
	/** The list holds the archived rows. */
	archived: boolean;
	rows: Row[];
	total: number;
	edit: boolean;
	/** The row whose impact line is open. */
	arming: string | undefined;
	busy: boolean;
	loading: boolean;
	onRetry: (() => void) | undefined;
	onRange: (range: Range) => void;
	onArm: (uuid: string | undefined) => void;
	/** Archive the row, or unarchive it with `false`. */
	onChange: (row: Row, archive: boolean) => void;
}) => {
	const narrow = useNarrow();
	const kind = () => KINDS[props.dimension];
	const counts = () => kind().threshold !== undefined;
	const columns = () => (counts() ? 6 : 5);
	const rowHeight = () => (narrow() ? ROW_HEIGHT.narrow : ROW_HEIGHT.wide);
	const [range, setRange] = createSignal<Range>({ start: 0, end: 0 });
	let body: HTMLTableSectionElement | undefined;
	let onScreen: Range = { start: 0, end: 0 };

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
			props.onRange(visible);
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
	createEffect(() => {
		const open = props.arming;
		if (open && !shown().some((row) => row.uuid === open)) {
			props.onArm(undefined);
		}
	});
	const above = () => range().start * rowHeight();
	const below = () => (props.rows.length - range().end) * rowHeight();
	const label = () =>
		`${props.archived ? "Archived" : "Active"} ${kind().label.toLowerCase()}`;

	return (
		<Table
			fold
			class="dm-table"
			aria-label={label()}
			aria-rowcount={props.total + 1}
			aria-busy={props.busy ? "true" : undefined}
		>
			<thead>
				<tr aria-rowindex={1}>
					<th scope="col">{kind().title}</th>
					<th scope="col">{kind().column}</th>
					<th scope="col">Created</th>
					<th scope="col">Last report</th>
					<Show when={counts()}>
						<th scope="col" data-align="end">
							Thresholds
						</th>
					</Show>
					<th scope="col">
						<span class="sr-only">Actions</span>
					</th>
				</tr>
			</thead>
			<tbody ref={body}>
				<Show when={above() > 0}>
					<Spacer height={above()} columns={columns()} />
				</Show>
				<For each={shown()}>
					{(row, index) => (
						<>
							<DimensionRow
								slug={props.slug}
								dimension={props.dimension}
								listArchived={props.archived}
								row={row}
								index={range().start + index()}
								edit={props.edit}
								arming={props.arming === row.uuid}
								onArm={() =>
									props.onArm(props.arming === row.uuid ? undefined : row.uuid)
								}
								onChange={(archive) => {
									props.onChange(row, archive);
									focusRow(body, row.uuid);
								}}
							/>
							<Show when={props.arming === row.uuid}>
								<tr class="dm-impact-row">
									<td colSpan={columns()} data-fold="full">
										<Impact
											name={row.name}
											text={archiveImpact(row.name, thresholdsOf(row))}
											confirm={`Archive ${row.name}`}
											onConfirm={() => {
												props.onChange(row, true);
												focusRow(body, row.uuid);
											}}
											onCancel={() => {
												props.onArm(undefined);
												focusRow(body, row.uuid);
											}}
										/>
									</td>
								</tr>
							</Show>
						</>
					)}
				</For>
				<Show when={below() > 0}>
					<Spacer height={below()} columns={columns()} />
				</Show>
				<Show when={props.loading}>
					{/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: a placeholder row has nothing to focus or read */}
					<tr class="dm-spacer dm-more" aria-hidden="true">
						<td colSpan={columns()}>
							<span class="ui-skeleton" data-size="text" />
						</td>
					</tr>
				</Show>
				<Show when={!props.loading && props.onRetry}>
					{(retry) => (
						<tr class="dm-more">
							<td colSpan={columns()}>
								<div class="dm-failed" role="alert">
									<span>
										These rows did not load: the Bencher API did not answer.
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

export default RowTable;

const Spacer = (props: { height: number; columns: number }) => (
	// biome-ignore lint/a11y/noAriaHiddenOnFocusable: a spacer row only holds the height of the rows not drawn
	<tr
		class="dm-spacer"
		aria-hidden="true"
		style={{ height: `${props.height}px` }}
	>
		<td colSpan={props.columns} />
	</tr>
);

const DimensionRow = (props: {
	slug: string;
	dimension: Dimension;
	listArchived: boolean;
	row: Row;
	index: number;
	edit: boolean;
	arming: boolean;
	onArm: () => void;
	onChange: (archive: boolean) => void;
}) => {
	const row = () => props.row;
	const thresholds = () => thresholdsOf(row());
	const last = () => {
		const at = row().last_report;
		return at === undefined ? "never" : shortDate(at);
	};
	/** The row changed state on this page: archived from Active, or unarchived from Archived. */
	const changed = () => Boolean(row().archived) !== props.listArchived;
	return (
		<tr
			class="dm-row"
			classList={{ "dm-dim": !props.listArchived && changed() }}
			aria-rowindex={props.index + 2}
			data-uuid={row().uuid}
		>
			<td data-fold="l1" class="dm-name">
				<a
					class="dm-link"
					href={`${NEXT_PROJECTS}/${props.slug}/${props.dimension}/${row().slug}`}
				>
					{row().name}
				</a>
				<Show when={startPoint(row())}>
					{(from) => <span class="dm-sub">from {from()}</span>}
				</Show>
			</td>
			<td data-fold="l2" class="dm-extra">
				<Extra dimension={props.dimension} row={row()} />
				<span class="ui-fold-label"> · {last()}</span>
			</td>
			<td data-fold="hide" class="muted">
				{shortDay(row().created)}
			</td>
			<td data-fold="hide">{last()}</td>
			<Show when={thresholds() !== undefined}>
				<td data-fold="n1" data-align="end">
					{thresholds()}
					<span class="ui-fold-label">
						{thresholds() === 1 ? " threshold" : " thresholds"}
					</span>
				</td>
			</Show>
			<td data-fold="act" class="dm-act">
				<Actions
					name={row().name}
					archived={row().archived}
					listArchived={props.listArchived}
					changed={changed()}
					edit={props.edit}
					arming={props.arming}
					onArm={props.onArm}
					onChange={props.onChange}
				/>
			</td>
		</tr>
	);
};

const startPoint = (row: Row) =>
	"start_point" in row ? row.start_point : undefined;

/** The column after the name: a branch's newest hash, a testbed's spec, a benchmark's variants, a measure's units. */
const Extra = (props: { dimension: Dimension; row: Row }): JSX.Element => {
	const row = props.row as Partial<
		JsonConsoleBranchRow &
			JsonConsoleTestbedRow &
			JsonConsoleBenchmarkRow &
			JsonConsoleMeasureRow
	>;
	switch (props.dimension) {
		case "branches":
			return <span class="mono muted">{row.hash?.slice(0, 7)}</span>;
		case "testbeds":
			return row.spec ? (
				<span>{row.spec}</span>
			) : (
				<span class="faint">no spec</span>
			);
		case "benchmarks":
			return (
				<span>
					{row.variants}
					<span class="ui-fold-label">
						{row.variants === 1 ? " variant" : " variants"}
					</span>
				</span>
			);
		case "measures":
			return <span>{row.units}</span>;
	}
};

/** The control that replaces the one used in a row keeps the focus in that row. */
export const focusRow = (body: HTMLElement | undefined, uuid: string) =>
	requestAnimationFrame(() =>
		body
			?.querySelector<HTMLElement>(
				`tr[data-uuid="${CSS.escape(uuid)}"] .dm-act button`,
			)
			?.focus(),
	);

/** Archive, or what just changed with its Undo, or when it was archived with Unarchive. */
export const Actions = (props: {
	name: string;
	archived: number | string | undefined;
	listArchived: boolean;
	changed: boolean;
	edit: boolean;
	arming: boolean;
	onArm: () => void;
	onChange: (archive: boolean) => void;
}) => (
	<Show
		when={props.edit}
		fallback={
			<Show when={props.archived}>
				{(at) => <span class="dm-status">Archived {shortDay(at())}</span>}
			</Show>
		}
	>
		<Show
			when={props.changed}
			fallback={
				<Show
					when={props.archived}
					fallback={
						<button
							type="button"
							class="lnk"
							aria-expanded={props.arming}
							aria-label={`Archive ${props.name}`}
							onClick={() => props.onArm()}
						>
							Archive
						</button>
					}
				>
					{(at) => (
						<>
							<span class="dm-status">Archived {shortDay(at())}</span>
							<span class="dotsep" aria-hidden="true">
								·
							</span>
							<button
								type="button"
								class="lnk"
								aria-label={`Unarchive ${props.name}`}
								onClick={() => props.onChange(false)}
							>
								Unarchive
							</button>
						</>
					)}
				</Show>
			}
		>
			<span class="dm-status">
				{props.listArchived ? "Unarchived just now" : "Archived just now"}
			</span>
			<span class="dotsep" aria-hidden="true">
				·
			</span>
			<button
				type="button"
				class="lnk"
				aria-label={`Undo, ${props.name}`}
				onClick={() => props.onChange(props.listArchived)}
			>
				Undo
			</button>
		</Show>
	</Show>
);

/** What archiving will do, said before it happens, with the confirm beside it. */
export const Impact = (props: {
	name: string;
	text: string;
	confirm: string;
	onConfirm: () => void;
	onCancel: () => void;
}) => {
	let confirm: HTMLButtonElement | undefined;
	onMount(() => confirm?.focus());
	return (
		<fieldset
			class="dm-impact"
			aria-label={`Archive ${props.name}`}
			onKeyDown={(event) => {
				if (event.key === "Escape") {
					props.onCancel();
				}
			}}
		>
			<Icon name="warning" class="dm-warn" />
			<span class="dm-impact-text">{props.text}</span>
			<Button
				size="sm"
				ref={(element: HTMLButtonElement) => {
					confirm = element;
				}}
				onClick={() => props.onConfirm()}
			>
				{props.confirm}
			</Button>
			<Button size="sm" onClick={() => props.onCancel()}>
				Cancel
			</Button>
		</fieldset>
	);
};
