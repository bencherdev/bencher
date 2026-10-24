import Button from "@bencherdev/ui/Button";
import { Show } from "solid-js";
import type {
	JsonConsoleRunReport,
	JsonConsoleRunSummary,
} from "../../types/bencher";
import { reportPath, runPath } from "../paths";
import { absoluteTime, duration } from "../reports/format";
import type { Alerts, Author, Dash } from "./cells";
import type { secondsTime } from "./format";
import type { RunsRow } from "./rows";
import StatusPill from "./StatusPill";
import { reportStatus, statusText } from "./status";

/** A row under an opened run, by its kind. */
const ChildRow = (props: {
	row: Exclude<RunsRow, { kind: "run" | "loading" }>;
	slug: string;
	now: number;
	index: number;
	/** Moving onto, focusing, or touching a row's link, before the click. */
	onReportIntent: (report: string) => void;
	onRunIntent: (run: JsonConsoleRunSummary, event?: PointerEvent) => void;
	/** A run's reports failed; asks for them again. */
	onRetry: (run: string) => void;
	parts: Parts;
}) => {
	const { row } = props;
	switch (row.kind) {
		case "report":
			return (
				<ReportRow
					slug={props.slug}
					report={row.report}
					now={props.now}
					index={props.index}
					onIntent={() => props.onReportIntent(row.report.uuid)}
					parts={props.parts}
				/>
			);
		case "all":
			return (
				<AllRow
					slug={props.slug}
					run={row.run.uuid}
					total={row.total}
					index={props.index}
					onIntent={(event) => props.onRunIntent(row.run, event)}
				/>
			);
		default:
			return (
				<FailedRow
					index={props.index}
					onRetry={() => props.onRetry(row.run.uuid)}
				/>
			);
	}
};

export default ChildRow;

/** What a report's row draws as its run's row does, handed over by the list so the chunk it shares with the run page stays whole. */
interface Parts {
	Alerts: typeof Alerts;
	Author: typeof Author;
	Dash: typeof Dash;
	secondsTime: typeof secondsTime;
}

/** An opened run's report in the Runs list's columns, opening its own page as the run page's row does. */
const ReportRow = (props: {
	slug: string;
	report: JsonConsoleRunReport;
	now: number;
	index: number;
	onIntent: () => void;
	parts: Parts;
}) => {
	const { Alerts, Author, Dash, secondsTime } = props.parts;
	const status = () => reportStatus(props.report, props.now, duration);
	// A job a runner has not taken has not started.
	const started = () =>
		props.report.job && props.report.job.started === undefined
			? undefined
			: secondsTime(props.report.start_time);
	return (
		<tr
			class="reports-row rn-row rn-child rowlink"
			aria-rowindex={props.index + 2}
		>
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
			<td class="rn-ord">{props.report.order}</td>
			<td data-fold="hide" />
			<td data-fold="hide" />
			<td data-fold="l1" class="reports-testbed">
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
					onPointerMove={(event) => {
						if (event.movementX || event.movementY) {
							props.onIntent();
						}
					}}
					onFocus={() => props.onIntent()}
					onTouchStart={() => props.onIntent()}
				>
					{props.report.testbed.name}
				</a>
			</td>
			<td data-fold="hide">{props.report.adapter}</td>
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

/** The way from an opened run's first reports to the rest of them, on its page. */
const AllRow = (props: {
	slug: string;
	run: string;
	total: number;
	index: number;
	onIntent: (event?: PointerEvent) => void;
}) => (
	<tr
		class="reports-row rn-row rn-child rowlink"
		aria-rowindex={props.index + 2}
	>
		<td data-fold="hide" />
		<td colSpan={9} data-fold="l1">
			<a
				class="rowlink-name"
				href={runPath(props.slug, props.run)}
				onPointerMove={props.onIntent}
				onFocus={() => props.onIntent()}
				onTouchStart={() => props.onIntent()}
			>
				All {props.total} reports
			</a>
		</td>
	</tr>
);

/** An opened run whose reports did not load, and a way to ask again. */
const FailedRow = (props: { index: number; onRetry: () => void }) => (
	<tr class="reports-row rn-row rn-child" aria-rowindex={props.index + 2}>
		<td data-fold="hide" />
		<td colSpan={9} data-fold="l1">
			<div class="reports-failed" role="alert">
				<span>These reports did not load</span>
				<Button size="sm" onClick={() => props.onRetry()}>
					Retry
				</Button>
			</div>
		</td>
	</tr>
);
