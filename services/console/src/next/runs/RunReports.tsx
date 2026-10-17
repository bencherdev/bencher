import Table from "@bencherdev/ui/Table";
import { For, Show } from "solid-js";
import type { JsonConsoleRunReport } from "../../types/bencher";
import { reportPath } from "../paths";
import { absoluteTime, duration } from "../reports/format";
import type { Range } from "../reports/rows";
import { MoreRows, Spacer } from "../reports/TableRows";
import { useVirtualRows } from "../reports/virtual";
import { Alerts, Author } from "./cells";
import { secondsTime } from "./format";

const COLUMNS = 8;

/** A run's reports in start order, drawing only the rows on screen. */
const RunReports = (props: {
	slug: string;
	reports: JsonConsoleRunReport[];
	/** Every report in the run, loaded or not. */
	total: number;
	onRange?: (range: Range) => void;
	loading?: boolean;
	onRetry?: (() => void) | undefined;
}) => {
	const rows = useVirtualRows({
		rows: () => props.reports,
		onRange: (range) => props.onRange?.(range),
	});
	return (
		<Table
			class="reports-table"
			aria-label="Reports in this run, by start"
			aria-rowcount={props.total + 1}
			data-fold=""
		>
			<thead>
				<tr aria-rowindex={1}>
					<th scope="col">#</th>
					<th scope="col">Testbed</th>
					<th scope="col">Adapter</th>
					<th scope="col">Started</th>
					<th scope="col" data-align="end">
						Lines
					</th>
					<th scope="col">Alerts</th>
					<th scope="col">Status</th>
					<th scope="col">By</th>
				</tr>
			</thead>
			<tbody ref={rows.body}>
				<Spacer height={rows.above()} columns={COLUMNS} />
				<For each={rows.shown()}>
					{(report, index) => (
						<ReportRow
							slug={props.slug}
							report={report}
							index={rows.start() + index()}
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
	index: number;
}) => {
	const started = () => secondsTime(props.report.start_time);
	return (
		<tr class="reports-row rn-row rowlink" aria-rowindex={props.index + 2}>
			<td class="rn-ord">{props.report.order}</td>
			<td data-fold="l1">
				<a
					class="reports-link rowlink-name"
					href={reportPath(props.slug, props.report.uuid)}
					aria-label={`${props.report.testbed.name}, report ${props.report.order}, started ${started()}`}
				>
					{props.report.testbed.name}
				</a>
			</td>
			<td data-fold="hide">{props.report.adapter}</td>
			<td data-fold="l2">
				<time
					datetime={new Date(props.report.start_time).toISOString()}
					title={absoluteTime(props.report.start_time)}
				>
					{started()}
				</time>
			</td>
			<td data-fold="n2" data-align="end">
				{props.report.lines}
				<span class="ui-fold-label">
					{props.report.lines === 1 ? " line" : " lines"}
				</span>
			</td>
			<td data-fold="n1">
				<Alerts alerts={props.report.alerts} />
			</td>
			<td data-fold="hide">
				{duration(props.report.end_time - props.report.start_time)}
			</td>
			<td data-fold="hide">
				<Show when={props.report.author}>
					{(author) => <Author author={author()} />}
				</Show>
			</td>
		</tr>
	);
};
