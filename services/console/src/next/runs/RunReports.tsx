import Table from "@bencherdev/ui/Table";
import { For, Show } from "solid-js";
import type { JsonConsoleRunReport } from "../../types/bencher";
import { reportPath } from "../paths";
import { absoluteTime, duration } from "../reports/format";
import type { Range } from "../reports/rows";
import { MoreRows, Spacer } from "../reports/TableRows";
import { useVirtualRows } from "../reports/virtual";
import { Alerts, Author, Dash } from "./cells";
import StatusPill from "./StatusPill";
import { secondsTime } from "./format";
import { onMoved } from "./Neighbor";
import { reportStatus, statusText } from "./status";

const COLUMNS = 8;

/** A run's reports in start order, drawing only the rows on screen. */
const RunReports = (props: {
	slug: string;
	reports: JsonConsoleRunReport[];
	/** Every report in the run, loaded or not. */
	total: number;
	now: number;
	onRange?: (range: Range) => void;
	/** Moving onto, focusing, or touching a report's link, before the click. */
	onIntent?: (uuid: string) => void;
	loading?: boolean;
	onRetry?: (() => void) | undefined;
}) => {
	const rows = useVirtualRows({
		rows: () => props.reports,
		onRange: (range) => props.onRange?.(range),
	});
	return (
		<Table
			class="reports-table rn-table"
			aria-label="Reports in this run, by start"
			aria-rowcount={props.total + 1}
			data-fold=""
		>
			<thead>
				<tr aria-rowindex={1}>
					{/* Every share but the testbed's is set, so the columns never follow the rows drawn. */}
					<th scope="col" style="width:4%">
						#
					</th>
					<th scope="col">Testbed</th>
					<th scope="col" style="width:13%">
						Adapter
					</th>
					<th scope="col" style="width:10%">
						Started
					</th>
					<th scope="col" style="width:7%" data-align="end">
						Lines
					</th>
					<th scope="col" style="width:21%">
						Alerts
					</th>
					<th scope="col" style="width:142px">
						Status
					</th>
					<th scope="col" style="width:15%">
						By
					</th>
				</tr>
			</thead>
			<tbody ref={rows.body}>
				<Spacer height={rows.above()} columns={COLUMNS} />
				<For each={rows.shown()}>
					{(report, index) => (
						<ReportRow
							slug={props.slug}
							report={report}
							now={props.now}
							index={rows.start() + index()}
							onIntent={() => props.onIntent?.(report.uuid)}
						/>
					)}
				</For>
				<Spacer height={rows.below()} columns={COLUMNS} />
				<MoreRows
					columns={COLUMNS}
					loading={props.loading ?? false}
					onRetry={props.onRetry}
					failed="These reports"
				/>
			</tbody>
		</Table>
	);
};

export default RunReports;

const ReportRow = (props: {
	slug: string;
	report: JsonConsoleRunReport;
	now: number;
	index: number;
	onIntent: () => void;
}) => {
	const status = () => reportStatus(props.report, props.now, duration);
	// A job a runner has not taken has not started.
	const started = () =>
		props.report.job && props.report.job.started === undefined
			? undefined
			: secondsTime(props.report.start_time);
	return (
		<tr class="reports-row rn-row rowlink" aria-rowindex={props.index + 2}>
			<td class="rn-ord">{props.report.order}</td>
			<td data-fold="l1">
				<a
					class="reports-link rowlink-name"
					href={reportPath(props.slug, props.report.uuid)}
					aria-label={[
						props.report.testbed.name,
						`report ${props.report.order}`,
						started() && `started ${started()}`,
						status().kind && statusText(status()),
					]
						.filter(Boolean)
						.join(", ")}
					onPointerMove={onMoved(() => props.onIntent())}
					onFocus={() => props.onIntent()}
					onTouchStart={() => props.onIntent()}
				>
					{props.report.testbed.name}
				</a>
			</td>
			<td data-fold="hide">{props.report.adapter}</td>
			<td data-fold="l2">
				<Show when={started()}>
					{(started) => (
						<time
							datetime={new Date(props.report.start_time).toISOString()}
							title={absoluteTime(props.report.start_time)}
						>
							{started()}
						</time>
					)}
				</Show>
			</td>
			<td data-fold={props.report.job ? "hide" : "n2"} data-align="end">
				<Show when={!props.report.job} fallback={<Dash />}>
					{props.report.lines}
					<span class="ui-fold-label">
						{props.report.lines === 1 ? " line" : " lines"}
					</span>
				</Show>
			</td>
			<td data-fold={props.report.job ? "hide" : "n1"}>
				<Show when={!props.report.job} fallback={<Dash />}>
					<Alerts alerts={props.report.alerts} />
				</Show>
			</td>
			<td class="rn-status" data-fold={props.report.job ? "n1" : "hide"}>
				<StatusPill status={status()} />
			</td>
			<td data-fold="hide">
				<Show when={props.report.author}>
					{(author) => <Author author={author()} />}
				</Show>
			</td>
		</tr>
	);
};
