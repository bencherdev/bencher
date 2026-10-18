import Table from "@bencherdev/ui/Table";
import { For, Show } from "solid-js";
import type {
	JsonConsoleAuthor,
	JsonConsoleRunSummary,
} from "../../types/bencher";
import { absoluteTime, duration, relativeTime } from "../reports/format";
import { runPath } from "../paths";
import type { Range } from "../reports/rows";
import { MoreRows, Spacer } from "../reports/TableRows";
import { useVirtualRows } from "../reports/virtual";
import { allNames, runHead } from "./format";
import { Alerts, Author, Dash } from "./cells";
import StatusPill from "./StatusPill";
import { landed, runStatus, statusText } from "./status";

const COLUMNS = 10;

/**
 * The runs, newest first, drawing only the rows on screen: rows above and
 * below them are spacers of the same height.
 */
const RunsTable = (props: {
	slug: string;
	runs: JsonConsoleRunSummary[];
	/** Every run the search matches, loaded or not. */
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
	const rows = useVirtualRows({
		rows: () => props.runs,
		onRange: (range) => props.onRange?.(range),
	});
	return (
		<Table
			class="reports-table rn-list"
			aria-label="Runs, newest first"
			aria-rowcount={props.total + 1}
			aria-busy={props.busy === true ? "true" : undefined}
			data-fold=""
		>
			<thead>
				<tr aria-rowindex={1}>
					{/* Every share but the branch's is set, so the columns never follow the rows drawn. */}
					<th scope="col" style="width:8.4%">
						When
					</th>
					<th scope="col">Branch</th>
					<th scope="col" style="width:7.6%">
						Head
					</th>
					<th scope="col" style="width:8%" data-align="end">
						Reports
					</th>
					<th scope="col" style="width:9.1%">
						Testbed
					</th>
					<th scope="col" style="width:8.7%">
						Adapter
					</th>
					<th scope="col" style="width:5.85%" data-align="end">
						Lines
					</th>
					<th scope="col" style="width:18%">
						Alerts
					</th>
					<th scope="col" style="width:142px">
						Status
					</th>
					<th scope="col" style="width:10.75%">
						By
					</th>
				</tr>
			</thead>
			<tbody ref={rows.body}>
				<Spacer height={rows.above()} columns={COLUMNS} />
				<For each={rows.shown()}>
					{(run, index) => (
						<RunRow
							slug={props.slug}
							run={run}
							now={props.now}
							index={rows.start() + index()}
						/>
					)}
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
}) => {
	const head = () => runHead(props.run);
	const absolute = () => absoluteTime(props.run.start_time);
	const status = () => runStatus(props.run, props.now, duration);
	const counted = () => landed(props.run.reports, props.run.jobs);
	// A run that raised alerts draws them with code of their own, which starts as the reader moves to it.
	const alertsAhead = (event?: PointerEvent) => {
		if (
			props.run.alerts.total > 0 &&
			(!event || event.movementX || event.movementY)
		) {
			import("./RunAlerts").catch(() => {});
		}
	};
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
				<a
					class="reports-link rowlink-name"
					href={runPath(props.slug, props.run.uuid)}
					aria-label={label()}
					onPointerMove={alertsAhead}
					onFocus={() => alertsAhead()}
					onTouchStart={() => alertsAhead()}
				>
					{props.run.branch.name}
				</a>
			</td>
			<td class="reports-hash">
				<Show when={!props.run.hash} fallback={head()}>
					<span class="rowlink-own" title={`Version ${props.run.number}`}>
						{head()}
					</span>
				</Show>
			</td>
			<td class="rn-n" data-align="end">
				<span class="ui-fold-label">· </span>
				{props.run.reports}
				<span class="ui-fold-label">
					{props.run.reports === 1 ? " report" : " reports"}
				</span>
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
