import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
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
import type { JsonReport } from "../../types/bencher";
import { reportPath } from "../paths";
import { useNarrow } from "../plot/narrow";
import {
	absoluteTime,
	creator,
	duration,
	lines,
	relativeTime,
	shortHash,
} from "./format";
import { ROW_HEIGHT } from "./query";
import { type Range, visibleRange } from "./rows";

/** Rows drawn above and below the screen, so a quick scroll never shows a gap. */
const OVERSCAN = 8;
const COLUMNS = 9;

/**
 * The reports, newest first, drawing only the rows on screen: rows above and
 * below them are spacers of the same height.
 */
const ReportsTable = (props: {
	slug: string;
	reports: JsonReport[];
	/** Every report the search matches, loaded or not. */
	total: number;
	now: number;
	/** The rows on screen changed, not counting those drawn either side. */
	onRange?: (range: Range) => void;
	/** The rows are a search's before the last, while the new one loads. */
	busy?: boolean;
	loading?: boolean;
	/** The next batch failed; asks for it again. */
	onRetry?: (() => void) | undefined;
}) => {
	const narrow = useNarrow();
	const [rowHeight, setRowHeight] = createSignal<number>(ROW_HEIGHT.wide);
	const [range, setRange] = createSignal<Range>({ start: 0, end: 0 });
	let body: HTMLTableSectionElement | undefined;
	let onScreen: Range = { start: 0, end: 0 };

	const measure = () => {
		if (!body) {
			return;
		}
		const rows = body.querySelectorAll<HTMLElement>("tr[aria-rowindex]");
		const [first, second] = rows;
		const drawn =
			first && second
				? second.getBoundingClientRect().top - first.getBoundingClientRect().top
				: first?.getBoundingClientRect().height;
		const height =
			drawn && drawn > 0
				? drawn
				: narrow()
					? ROW_HEIGHT.narrow
					: ROW_HEIGHT.wide;
		setRowHeight(height);
		// Drawn at the expected height until a row exists to measure.
		if (!drawn && props.reports.length > 0) {
			schedule();
		}
		const place = {
			top: body.getBoundingClientRect().top,
			viewport: window.innerHeight,
			rowHeight: height,
			count: props.reports.length,
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
	createEffect(on([() => props.reports.length, narrow], measure));

	const shown = createMemo(() =>
		props.reports.slice(range().start, range().end),
	);
	const above = () => range().start * rowHeight();
	const below = () => (props.reports.length - range().end) * rowHeight();

	return (
		<Table
			class="reports-table"
			aria-label="Reports, newest first"
			aria-rowcount={props.total + 1}
			aria-busy={props.busy === true ? "true" : undefined}
			data-fold=""
		>
			<thead>
				<tr aria-rowindex={1}>
					<th scope="col">When</th>
					<th scope="col">Branch</th>
					<th scope="col">Head</th>
					<th scope="col">Testbed</th>
					<th scope="col">Adapter</th>
					<th scope="col" data-align="end">
						Lines
					</th>
					<th scope="col">Alerts</th>
					<th scope="col" data-align="end">
						Took
					</th>
					<th scope="col">By</th>
				</tr>
			</thead>
			<tbody ref={body}>
				<Show when={above() > 0}>
					<Spacer height={above()} />
				</Show>
				<For each={shown()}>
					{(report, index) => (
						<ReportRow
							slug={props.slug}
							report={report}
							now={props.now}
							index={range().start + index()}
						/>
					)}
				</For>
				<Show when={below() > 0}>
					<Spacer height={below()} />
				</Show>
				<Show when={props.loading}>
					{/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: a placeholder row has nothing to focus or read */}
					<tr class="reports-spacer reports-more" aria-hidden="true">
						<td colSpan={COLUMNS}>
							<span class="ui-skeleton" data-size="text" />
						</td>
					</tr>
				</Show>
				<Show when={!props.loading && props.onRetry}>
					{(retry) => (
						<tr class="reports-more">
							<td colSpan={COLUMNS}>
								<div class="reports-failed" role="alert">
									<span>
										These reports did not load: the Bencher API did not answer.
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

export default ReportsTable;

const Spacer = (props: { height: number }) => (
	// biome-ignore lint/a11y/noAriaHiddenOnFocusable: a spacer row only holds the height of the rows not drawn
	<tr
		class="reports-spacer"
		aria-hidden="true"
		style={{ height: `${props.height}px` }}
	>
		<td colSpan={COLUMNS} />
	</tr>
);

const ReportRow = (props: {
	slug: string;
	report: JsonReport;
	now: number;
	index: number;
}) => {
	const start = () => Date.parse(props.report.start_time);
	const hash = () => shortHash(props.report.branch.head.version?.hash);
	const absolute = () => absoluteTime(start());
	const label = () =>
		[
			`Report on ${props.report.branch.name}`,
			props.report.testbed.name,
			props.report.adapter,
			absolute(),
			hash(),
		]
			.filter(Boolean)
			.join(", ");
	return (
		<tr class="reports-row" aria-rowindex={props.index + 2}>
			<td data-fold="n2" class="reports-when">
				<time datetime={props.report.start_time} title={absolute()}>
					{relativeTime(start(), props.now)}
				</time>
			</td>
			<td data-fold="l1" class="reports-branch">
				<a
					class="reports-link"
					href={reportPath(props.slug, props.report.uuid)}
					aria-label={label()}
				>
					{props.report.branch.name}
				</a>
			</td>
			<td class="reports-hash">{hash()}</td>
			<td data-fold="l2" class="reports-testbed">
				{props.report.testbed.name}
			</td>
			<td class="reports-adapter">{props.report.adapter}</td>
			<td data-fold="hide" data-align="end">
				{lines(props.report)}
			</td>
			<td data-fold="n1">
				<Alerts report={props.report} />
			</td>
			<td data-fold="hide" data-align="end">
				{duration(Date.parse(props.report.end_time) - start())}
			</td>
			<td data-fold="hide">
				<By report={props.report} />
			</td>
		</tr>
	);
};

const Alerts = (props: { report: JsonReport }) => {
	const active = () => props.report.counts?.alerts.active ?? 0;
	const total = () => props.report.counts?.alerts.total ?? 0;
	return (
		<span class="reports-alerts">
			<Show when={active() > 0}>
				<span class="reports-active">
					<span class="reports-dot" aria-hidden="true" />
					{active()} active
				</span>{" "}
			</Show>
			<Show when={total() > 0} fallback={<span class="reports-quiet">0</span>}>
				<span class="reports-quiet">{total()} total</span>
			</Show>
		</span>
	);
};

const By = (props: { report: JsonReport }) => (
	<Show when={creator(props.report)}>
		{(who) => (
			<span class="reports-by">
				<span
					role="img"
					aria-label={who().kind === "key" ? "Project key" : "User"}
				>
					<Icon name={who().kind === "key" ? "key" : "user"} />
				</span>
				{who().name}
			</span>
		)}
	</Show>
);
