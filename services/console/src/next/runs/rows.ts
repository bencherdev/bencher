import type {
	JsonConsoleRun,
	JsonConsoleRunReport,
	JsonConsoleRunSummary,
} from "../../types/bencher";
import type { Range } from "../reports/rows";
import { RUN_REPORTS } from "./status";

/**
 * A row of the Runs list: a run, or under a run opened to its reports, one of
 * them, a placeholder for one not yet read, the way to the rest on its page,
 * or a read that failed.
 */
export type RunsRow =
	| { kind: "run"; run: JsonConsoleRunSummary }
	| { kind: "report"; report: JsonConsoleRunReport }
	| { kind: "loading" }
	| { kind: "all"; run: JsonConsoleRunSummary; total: number }
	| { kind: "failed"; run: JsonConsoleRunSummary };

/** An opened run's read of its first reports, as far as it got. */
export interface RunReportsRead {
	data?: JsonConsoleRun | undefined;
	isError?: boolean;
}

/**
 * The Runs list's rows, each opened run followed by its own. A run or report
 * the cache answers with unchanged keeps its row, and so does every other row
 * whose drawing is unchanged, so the table keeps what it drew.
 */
export const createRunsRows = () => {
	const runRows = new WeakMap<JsonConsoleRunSummary, RunsRow>();
	const reportRows = new WeakMap<JsonConsoleRunReport, RunsRow>();
	let ownRows = new Map<string, RunsRow>();
	return (
		runs: readonly JsonConsoleRunSummary[],
		open: ReadonlySet<string>,
		read: (run: string) => RunReportsRead | undefined,
	): RunsRow[] => {
		const kept = new Map<string, RunsRow>();
		const own = (key: string, make: () => RunsRow) => {
			const row = ownRows.get(key) ?? make();
			kept.set(key, row);
			return row;
		};
		const rows = runs.flatMap((run) => {
			const row = runRows.get(run) ?? { kind: "run", run };
			runRows.set(run, row);
			if (!open.has(run.uuid)) {
				return [row];
			}
			// The way to the rest reads the run's alerts only to start their code.
			const all = (total: number) =>
				own(`${run.uuid} all ${total} ${run.alerts.total > 0}`, () => ({
					kind: "all",
					run,
					total,
				}));
			const { data, isError } = read(run.uuid) ?? {};
			if (data) {
				const reports = data.reports.map((report) => {
					const row = reportRows.get(report) ?? { kind: "report", report };
					reportRows.set(report, row);
					return row;
				});
				return data.total > reports.length
					? [row, ...reports, all(data.total)]
					: [row, ...reports];
			}
			if (isError) {
				return [
					row,
					own(`${run.uuid} failed`, () => ({ kind: "failed", run })),
				];
			}
			const loading = Array.from(
				{ length: Math.min(run.reports, RUN_REPORTS) },
				(_, index) =>
					own(`${run.uuid} loading ${index}`, () => ({ kind: "loading" })),
			);
			return run.reports > RUN_REPORTS
				? [row, ...loading, all(run.reports)]
				: [row, ...loading];
		});
		ownRows = kept;
		return rows;
	};
};

/** The runs whose own rows start in `range`, a range of rows. */
export const runsIn = (rows: readonly RunsRow[], range: Range): Range => {
	const before = (end: number) =>
		rows.slice(0, end).filter((row) => row.kind === "run").length;
	return { start: before(range.start), end: before(range.end) };
};
