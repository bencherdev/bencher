import { describe, expect, test } from "vitest";
import type { JsonConsoleRunReport } from "../../types/bencher";
import { type RunsRow, createRunsRows, runsIn } from "./rows";
import { runFixture, runPageFixture, runReportFixture } from "./testing";

const kinds = (rows: RunsRow[]) =>
	rows.map((row) => (row.kind === "report" ? row.report.order : row.kind));

const reportsOf = (count: number, total = count) =>
	runPageFixture(
		Array.from({ length: count }, (_, index) => runReportFixture(index)),
		{ total },
	);

describe("createRunsRows", () => {
	const [first, second, third] = [
		runFixture(0, { reports: 3 }),
		runFixture(1, { reports: 12 }),
		runFixture(2, { reports: 2 }),
	] as const;
	const runs = [first, second, third];

	// Kills a closed run drawn with its reports, and runs out of their order.
	test("closed runs are one row each, newest first", () => {
		const rows = createRunsRows()(runs, new Set(), () => undefined);
		expect(rows.map((row) => row.kind === "run" && row.run)).toEqual(runs);
	});

	// Kills an opened run that waits for its reports to hold their place, one
	// that holds more than its first reports' place, and an All row that waits
	// for the reports though the run already counts them.
	test("an opened run not yet read holds a row for each of its first reports, then All", () => {
		const rows = createRunsRows()(
			runs,
			new Set([first.uuid, second.uuid]),
			() => undefined,
		);
		expect(kinds(rows)).toEqual([
			"run",
			...Array(3).fill("loading"),
			"run",
			...Array(8).fill("loading"),
			"all",
			"run",
		]);
		expect(rows.at(-2)).toMatchObject({ kind: "all", total: 12 });
	});

	// Kills a way to the rest drawn for a run that lists every report it has.
	test("an opened run of exactly its first reports has no row for the rest", () => {
		const eight = runFixture(0, { reports: 8 });
		const rows = createRunsRows();
		expect(
			kinds(rows([eight], new Set([eight.uuid]), () => undefined)),
		).toEqual(["run", ...Array(8).fill("loading")]);
		expect(
			kinds(
				rows([eight], new Set([eight.uuid]), () => ({ data: reportsOf(8) })),
			),
		).toEqual(["run", 1, 2, 3, 4, 5, 6, 7, 8]);
	});

	// Kills reports drawn from the summary's count once the run is read, an All
	// row for a run whose every report is drawn, and one that counts the summary.
	test("an opened run that was read draws its reports in order, the read answer winning", () => {
		const read = new Map([
			[first.uuid, { data: reportsOf(2) }],
			[second.uuid, { data: reportsOf(8, 13) }],
		]);
		const rows = createRunsRows()(
			runs,
			new Set([first.uuid, second.uuid]),
			(run) => read.get(run),
		);
		expect(kinds(rows)).toEqual([
			"run",
			1,
			2,
			"run",
			1,
			2,
			3,
			4,
			5,
			6,
			7,
			8,
			"all",
			"run",
		]);
		expect(rows.at(-2)).toMatchObject({ kind: "all", total: 13 });
	});

	// Kills a failed read drawn as rows that never fill, and one whose error
	// hides reports an earlier read already drew.
	test("a failed read is one row, unless an earlier read answered", () => {
		const rows = createRunsRows()(
			runs,
			new Set([first.uuid, third.uuid]),
			(run) =>
				run === first.uuid
					? { isError: true }
					: { data: reportsOf(2), isError: true },
		);
		expect(kinds(rows)).toEqual(["run", "failed", "run", "run", 1, 2]);
		expect(rows[1]).toEqual({ kind: "failed", run: first });
	});

	// Kills rows built anew on every change, which the table would draw again,
	// taking the focus and the hover with them.
	test("the same runs and reports keep the same rows", () => {
		const rowsOf = createRunsRows();
		const read = { data: reportsOf(2) };
		const open = new Set([first.uuid, second.uuid]);
		const before = rowsOf(runs, open, (run) =>
			run === first.uuid ? read : undefined,
		);
		const after = rowsOf([...runs], new Set(open), (run) =>
			run === first.uuid ? { ...read } : undefined,
		);
		expect(after).toHaveLength(before.length);
		for (const [index, row] of after.entries()) {
			expect(row).toBe(before[index]);
		}
	});

	// Kills an opened run's rows kept by the whole answer, or by the run's
	// summary, so a poll that rewrites either draws every report again,
	// unchanged ones included, and one that keeps a report whose answer changed.
	test("a poll that changes one report keeps the rest of the run's rows", () => {
		const rowsOf = createRunsRows();
		const read = reportsOf(8, 12);
		const open = new Set([second.uuid]);
		const before = rowsOf(runs, open, () => ({ data: read }));
		const changed = { ...read.reports[1], lines: 7 } as JsonConsoleRunReport;
		const after = rowsOf([first, { ...second }, third], open, () => ({
			data: {
				...read,
				reports: read.reports.map((report, index) =>
					index === 1 ? changed : report,
				),
			},
		}));
		expect(after.map((row, index) => row === before[index])).toEqual([
			true,
			false,
			true,
			false,
			...Array(6).fill(true),
			true,
			true,
		]);
		expect(after[3]).toEqual({ kind: "report", report: changed });
	});

	// Kills a way to the rest that keeps the summary's count once the read
	// answers with another, a report having landed in between.
	test("the way to the rest counts what the read answers once it does", () => {
		const rowsOf = createRunsRows();
		const open = new Set([second.uuid]);
		expect(rowsOf(runs, open, () => undefined).at(-2)).toMatchObject({
			kind: "all",
			total: 12,
		});
		expect(
			rowsOf(runs, open, () => ({ data: reportsOf(8, 13) })).at(-2),
		).toMatchObject({ kind: "all", total: 13 });
	});

	// Kills a way to the rest kept from before its run raised an alert, which
	// then never starts the code its page draws the alerts with.
	test("the way to the rest is drawn again once its run raises an alert", () => {
		const rowsOf = createRunsRows();
		const open = new Set([second.uuid]);
		const read = () => ({ data: reportsOf(8, 12) });
		const before = rowsOf(runs, open, read).at(-2);
		const alerting = { ...second, alerts: { total: 1, active: 1 } };
		const after = rowsOf([first, alerting, third], open, read).at(-2);
		expect(after).not.toBe(before);
		expect(after).toEqual({ kind: "all", run: alerting, total: 12 });
	});

	// Kills placeholders drawn again as the list asks again, which moves
	// nothing yet costs a redraw of every row held.
	test("placeholders keep their rows while the run waits for its reports", () => {
		const rowsOf = createRunsRows();
		const open = new Set([second.uuid]);
		const before = rowsOf(runs, open, () => undefined);
		const after = rowsOf([first, { ...second }, third], open, () => undefined);
		expect(after.slice(2, 11)).toEqual(before.slice(2, 11));
		for (const [index, row] of after.slice(2, 11).entries()) {
			expect(row).toBe(before[index + 2]);
		}
	});
});

// Kills a next batch asked for by rows rather than runs, which comes early
// while a run is open and an opened run's rows count as runs loaded.
test("runsIn counts the runs before each end of a range of rows", () => {
	const rows = createRunsRows()(
		[runFixture(0, { reports: 4 }), runFixture(1), runFixture(2)],
		new Set([runFixture(0).uuid]),
		() => undefined,
	);
	expect(runsIn(rows, { start: 0, end: 5 })).toEqual({ start: 0, end: 1 });
	expect(runsIn(rows, { start: 2, end: 6 })).toEqual({ start: 1, end: 2 });
	expect(runsIn(rows, { start: 5, end: 7 })).toEqual({ start: 1, end: 3 });
});
