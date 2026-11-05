import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import {
	For,
	Show,
	Suspense,
	createEffect,
	createMemo,
	createSignal,
	lazy,
} from "solid-js";
import type {
	JsonConsoleAuthor,
	JsonConsoleRunSummary,
} from "../../types/bencher";
import { absoluteTime, duration, relativeTime } from "../reports/format";
import { runPath } from "../paths";
import type { Range } from "../reports/rows";
import { MoreRows, Spacer } from "../reports/TableRows";
import { useVirtualRows } from "../reports/virtual";
import { allNames, runHead, secondsTime } from "./format";
import { Alerts, Author, Dash } from "./cells";
import {
	type RunReportsRead,
	type RunsRow,
	createRunsRows,
	runsIn,
} from "./rows";
import StatusPill from "./StatusPill";
import { landed, runStatus, statusText } from "./status";

const COLUMNS = 10;
/** The width below which the Branch column holds under 16 characters. */
const RUNS_FOLD = 1145;

const NONE: ReadonlySet<string> = new Set();

const PARTS = { Alerts, Author, Dash, secondsTime };

/** An opened run's rows draw with code of their own, which starts as the reader moves to a run. */
export const ChildRow = lazy(() => import("./RunChildren"));

/**
 * The runs, newest first, each opened run's first reports under it, drawing
 * only the rows on screen: rows above and below them are spacers of the same
 * height.
 */
const RunsTable = (props: {
	slug: string;
	runs: JsonConsoleRunSummary[];
	/** Every run the search matches, loaded or not. */
	total: number;
	now: number;
	/** The runs on screen changed, not counting those drawn either side. */
	onRange?: (range: Range) => void;
	/** The rows are a search's before the last, while the new one loads. */
	busy?: boolean;
	loading?: boolean;
	/** The next batch failed; asks for it again. */
	onRetry?: (() => void) | undefined;
	/** The runs opened to their reports. */
	open?: ReadonlySet<string>;
	onToggle?: (run: string) => void;
	/** An opened run's read of its first reports. */
	reports?: (run: string) => RunReportsRead | undefined;
	/** An opened run's reports failed; asks for them again. */
	onRetryReports?: (run: string) => void;
	/** The pointer moving onto a run's row, or focus or a touch on its toggle, before the click. */
	onRunIntent?: (run: string) => void;
	/** Moving onto, focusing, or touching a report's link, before the click. */
	onReportIntent?: (report: string) => void;
	/** Moving onto, focusing, or touching a way to a run's page, its head or the row for the rest of its reports, before the click. */
	onRunPageIntent?: (run: JsonConsoleRunSummary) => void;
}) => {
	const rowsOf = createRunsRows();
	const flat = createMemo(() =>
		rowsOf(props.runs, props.open ?? NONE, (run) => props.reports?.(run)),
	);
	const [onScreen, setOnScreen] = createSignal<Range>();
	const rows = useVirtualRows({ rows: flat, onRange: setOnScreen });
	createEffect(() => {
		const range = onScreen();
		if (range) {
			props.onRange?.(runsIn(flat(), range));
		}
	});
	const toPage = (run: JsonConsoleRunSummary) => (event?: PointerEvent) => {
		if (!event || event.movementX || event.movementY) {
			props.onRunPageIntent?.(run);
		}
	};
	const draw = (row: RunsRow, index: () => number) => {
		if (row.kind === "run") {
			return (
				<RunRow
					slug={props.slug}
					run={row.run}
					now={props.now}
					index={index()}
					open={(props.open ?? NONE).has(row.run.uuid)}
					onToggle={() => props.onToggle?.(row.run.uuid)}
					onIntent={() => {
						ChildRow.preload();
						props.onRunIntent?.(row.run.uuid);
					}}
					onPageIntent={toPage(row.run)}
				/>
			);
		}
		if (row.kind === "loading") {
			return <LoadingRow index={index()} />;
		}
		return (
			<Suspense fallback={<LoadingRow index={index()} />}>
				<ChildRow
					row={row}
					slug={props.slug}
					now={props.now}
					index={index()}
					onReportIntent={(report) => props.onReportIntent?.(report)}
					onRunIntent={(run, event) => toPage(run)(event)}
					onRetry={(run) => props.onRetryReports?.(run)}
					parts={PARTS}
				/>
			</Suspense>
		);
	};
	return (
		<Table
			class="reports-table rn-list"
			aria-label="Runs, newest first"
			aria-rowcount={props.total + flat().length - props.runs.length + 1}
			aria-busy={props.busy === true ? "true" : undefined}
			fold={RUNS_FOLD}
			onLines={rows.onLines}
		>
			<thead>
				<tr aria-rowindex={1}>
					{/* Every share but the branch's is set, so the columns never follow the rows drawn. */}
					<th scope="col" style="width:7.3%">
						When
					</th>
					<th scope="col">Branch</th>
					<th scope="col" style="width:6.4%">
						Head
					</th>
					<th scope="col" style="width:6.8%" data-align="end">
						Reports
					</th>
					<th scope="col" style="width:13.3%">
						Testbed
					</th>
					<th scope="col" style="width:8.9%">
						Adapter
					</th>
					<th scope="col" style="width:4.9%" data-align="end">
						Lines
					</th>
					<th scope="col" style="width:16.2%">
						Alerts
					</th>
					<th scope="col" style="width:142px">
						Status
					</th>
					<th scope="col" style="width:10.6%">
						By
					</th>
				</tr>
			</thead>
			<tbody ref={rows.body}>
				<Spacer height={rows.above()} columns={COLUMNS} />
				<For each={rows.shown()}>
					{(row, index) => draw(row, () => rows.start() + index())}
				</For>
				<Spacer height={rows.below()} columns={COLUMNS} />
				<MoreRows
					columns={COLUMNS}
					loading={props.loading ?? false}
					onRetry={props.onRetry}
					failed="These runs"
				/>
			</tbody>
		</Table>
	);
};

export default RunsTable;

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;

const RunRow = (props: {
	slug: string;
	run: JsonConsoleRunSummary;
	now: number;
	index: number;
	open: boolean;
	onToggle: () => void;
	onIntent: () => void;
	onPageIntent: (event?: PointerEvent) => void;
}) => {
	const head = () => runHead(props.run);
	const absolute = () => absoluteTime(props.run.start_time);
	const status = () => runStatus(props.run, props.now, duration);
	const counted = () => landed(props.run.reports, props.run.jobs);
	// The lists the row shows as one name and a count, whole, for a reader without a pointer.
	const label = () =>
		[
			props.run.branch.name,
			`run ${head()}`,
			plural(props.run.reports, "report"),
			allNames(props.run.testbeds, props.run.testbed_count),
			allNames(props.run.adapters, props.run.adapter_count),
			props.run.author_count > 0 &&
				`by ${allNames(
					props.run.authors.map((author) => author.name),
					props.run.author_count,
				)}`,
			absolute(),
			status().kind && statusText(status()),
		]
			.filter(Boolean)
			.join(", ");
	return (
		<tr class="reports-row rowlink" aria-rowindex={props.index + 2}>
			<td data-fold="n2" class="reports-when">
				<time
					datetime={new Date(props.run.start_time).toISOString()}
					title={absolute()}
				>
					{relativeTime(props.run.start_time, props.now)}
				</time>
			</td>
			<td data-fold="l1" class="reports-branch">
				<button
					type="button"
					class="rn-toggle rowlink-name"
					aria-label={label()}
					aria-expanded={props.open}
					onClick={() => props.onToggle()}
					onPointerMove={(event) => {
						if (event.movementX || event.movementY) {
							props.onIntent();
						}
					}}
					onFocus={() => props.onIntent()}
					onTouchStart={() => props.onIntent()}
				>
					{props.run.branch.name}
				</button>
			</td>
			<td class="reports-hash">
				<a
					href={runPath(props.slug, props.run.uuid)}
					title={props.run.hash ? undefined : `Version ${props.run.number}`}
					onPointerMove={(event) => props.onPageIntent(event)}
					onFocus={() => props.onPageIntent()}
					onTouchStart={() => props.onPageIntent()}
				>
					{head()}
				</a>
			</td>
			<td class="rn-n" data-align="end">
				<span class="ui-fold-label">· </span>
				{props.run.reports}
				<span class="ui-fold-label">
					{props.run.reports === 1 ? " report" : " reports"}
				</span>{" "}
				<Icon name="chevron-down" class="rn-caret muted" />
			</td>
			<td data-fold="l2" class="reports-testbed">
				<First names={props.run.testbeds} count={props.run.testbed_count} />
			</td>
			<td data-fold="hide" class="reports-adapter">
				<First names={props.run.adapters} count={props.run.adapter_count} />
			</td>
			<td data-fold="hide" data-align="end">
				<Show when={counted()} fallback={<Dash />}>
					{props.run.lines}
				</Show>
			</td>
			<td data-fold={status().kind ? "hide" : "n1"}>
				<Show when={counted()} fallback={<Dash />}>
					<Alerts alerts={props.run.alerts} />
				</Show>
			</td>
			<td class="rn-status" data-fold={status().kind ? "n1" : "hide"}>
				<StatusPill status={status()} />
			</td>
			<td data-fold="hide">
				<By authors={props.run.authors} count={props.run.author_count} />
			</td>
		</tr>
	);
};

/** A report of an opened run not yet read, holding its row's place with a placeholder where its testbed will be. */
const LoadingRow = (props: { index: number }) => (
	<tr
		class="reports-row rn-row rn-child"
		aria-rowindex={props.index + 2}
		aria-busy="true"
	>
		<td data-fold="hide" />
		<td data-fold="hide" colSpan={3} />
		<td data-fold="l1">
			<Skeleton size="text" />
		</td>
		<td data-fold="hide" colSpan={5} />
	</tr>
);

/** The first name, cut short before a muted count of the rest whose hover lists them all. */
const First = (props: { names: readonly string[]; count: number }) => (
	<span class="rn-first">
		<span class="rn-name">{props.names[0]}</span>
		<More names={props.names} count={props.count} />
	</span>
);

const More = (props: { names: readonly string[]; count: number }) => (
	<Show when={props.count > 1}>
		<span
			class="rn-more rowlink-own"
			title={allNames(props.names, props.count)}
		>
			+{props.count - 1}
		</span>
	</Show>
);

const By = (props: { authors: JsonConsoleAuthor[]; count: number }) => (
	<Show when={props.authors[0]}>
		{(first) => (
			<Author author={first()}>
				<More
					names={props.authors.map((author) => author.name)}
					count={props.count}
				/>
			</Author>
		)}
	</Show>
);
