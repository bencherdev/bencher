import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonConsoleJob,
	JsonConsoleRunJobs,
	JsonConsoleRunSummary,
} from "../../types/bencher";
import type { Range } from "../reports/rows";
import {
	columnEdges,
	expectNameStartsWithText,
	expectRowOpens,
} from "../ui/testing";
import type { RunReportsRead } from "./rows";
import RunsTable from "./RunsTable";
import {
	cutCells,
	cutHeaders,
	likelyReport,
	likelyRuns,
	runFixture,
	runPageFixture,
	runReportFixture,
} from "./testing";

const NOW = Date.parse("2026-09-14T00:00:00Z");
const RUN =
	"/console/projects/hashbrown/runs/00000000-0000-4000-8000-000000000000";
const REPORTS = "/console/projects/hashbrown/reports";

let dispose: (() => void) | undefined;
const ranges: Range[] = [];
const toggled: string[] = [];
const retried: string[] = [];
const intents: string[] = [];
let setReads: (reads: Record<string, RunReportsRead>) => void = () => {};

/** The table with `opened` open, each toggle opening or closing its run, and `reads` as each opened run's read of its reports. */
const mount = (
	runs: JsonConsoleRunSummary[],
	total = runs.length,
	{
		opened = [],
		reads = {},
	}: { opened?: string[]; reads?: Record<string, RunReportsRead> } = {},
) => {
	for (const list of [ranges, toggled, retried, intents]) {
		list.length = 0;
	}
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(() => {
		const [open, setOpen] = createSignal<ReadonlySet<string>>(new Set(opened));
		const [read, setRead] = createSignal(reads);
		setReads = setRead;
		return (
			<RunsTable
				slug="hashbrown"
				runs={runs}
				total={total}
				now={NOW}
				onRange={(range) => ranges.push(range)}
				open={open()}
				onToggle={(run) => {
					toggled.push(run);
					const next = new Set(open());
					if (!next.delete(run)) {
						next.add(run);
					}
					setOpen(next);
				}}
				reports={(run) => read()[run]}
				onRetryReports={(run) => retried.push(run)}
				onRunIntent={(run) => intents.push(`run ${run}`)}
				onReportIntent={(report) => intents.push(`report ${report}`)}
				onRunPageIntent={(run) => intents.push(`page ${run.uuid}`)}
			/>
		);
	}, root);
};

/** A run's read of its first reports: `count` of them, of `total` in all. */
const readOf = (count: number, total = count, fields = {}) => ({
	data: runPageFixture(
		Array.from({ length: count }, (_, index) => runReportFixture(index)),
		{ total, ...fields },
	),
});

const table = () => page.getByRole("table", { name: "Runs, newest first" });
const drawn = () =>
	[...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]")].map(
		(row) => Number(row.getAttribute("aria-rowindex")),
	);
const cells = (row: number) =>
	[...document.querySelectorAll(`tr[aria-rowindex="${row}"] td`)].map((cell) =>
		cell.textContent?.replace(/\s+/g, " ").trim(),
	);
/** What a pointer resting on the middle of `element`'s text would show on hover: the title it reaches. */
const hovered = (element: Element) => {
	const text = document.createRange();
	text.selectNodeContents(element);
	const box = text.getBoundingClientRect();
	return document
		.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2)
		?.closest("[title]")
		?.getAttribute("title");
};
/** Each `+N` of a row, with what the pointer reaches over it. */
const more = (row: number) =>
	[...document.querySelectorAll(`tr[aria-rowindex="${row}"] .rn-more`)].map(
		(count) => [count.textContent, hovered(count)],
	);

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	window.scrollTo({ top: 0, behavior: "instant" });
});

// Kills a list that draws every row it holds, one that does not follow the
// scroll, and spacers that leave the page shorter or longer than its rows.
test("draws only the rows on screen and follows the scroll", async () => {
	await page.viewport(1280, 720);
	mount(
		Array.from({ length: 500 }, (_, index) => runFixture(index)),
		2_000,
	);
	await expect.element(table()).toHaveAttribute("aria-rowcount", "2001");
	await expect.poll(() => drawn().length).toBeGreaterThan(10);
	expect(drawn().length).toBeLessThan(40);
	expect(drawn()[0]).toBe(2);

	const body = document.querySelector("tbody") as HTMLElement;
	const height = body.getBoundingClientRect().height;
	expect(height).toBeGreaterThanOrEqual(500 * 39);
	expect(height).toBeLessThanOrEqual(500 * 41 + 40);

	window.scrollTo({
		top: body.getBoundingClientRect().top + 300 * 40,
		behavior: "instant",
	});
	await expect.poll(() => drawn().includes(302)).toBe(true);
	expect(drawn()).not.toContain(2);
	expect(drawn().length).toBeLessThan(40);
	const onScreen = ranges.at(-1);
	expect(onScreen?.start).toBeGreaterThanOrEqual(299);
	expect(onScreen?.start).toBeLessThanOrEqual(300);
	expect((onScreen?.end ?? 0) - (onScreen?.start ?? 0)).toBeLessThanOrEqual(20);
});

// Kills cells that read the wrong field, a run's time taken from anything but
// its first start and last end, a key named as a person, an alerts cell that
// drops the active count or the total, a head that links anything but the
// run, and a toggle whose name leaves out the row.
test("a row reads its run", async () => {
	await page.viewport(1280, 720);
	mount([
		runFixture(0, {
			reports: 8,
			testbeds: ["ubuntu-latest", "macos-latest"],
			testbed_count: 2,
			adapters: ["magic"] as never,
			authors: [{ kind: "key", name: "GitHub Actions" }] as never,
			lines: 96,
			end_time: Date.parse("2026-09-13T21:30:08Z"),
			alerts: { total: 3, active: 1 },
		}),
		runFixture(1),
	]);

	await expect
		.element(table().getByRole("row").nth(1).getByRole("cell").nth(0))
		.toHaveTextContent("2h ago");
	expect(cells(2)).toEqual([
		"2h ago",
		"main",
		"9c1f2e4",
		"· 8 reports",
		"ubuntu-latest+1",
		"magic",
		"96",
		"1 active 3 total",
		"14m 08s",
		"GitHub Actions",
	]);
	expect(cells(3)).toEqual([
		"3h ago",
		"main",
		"9c1f2e4",
		"· 1 report",
		"ubuntu-latest",
		"json",
		"36",
		"0",
		"2m 00s",
		"Muriel Bagge",
	]);
	await expect
		.element(
			table().getByRole("row").nth(1).getByRole("img", { name: "Project key" }),
		)
		.toBeVisible();
	await expect
		.element(table().getByRole("row").nth(2).getByRole("img", { name: "User" }))
		.toBeVisible();

	const head = table().getByRole("row").nth(1).getByRole("link");
	await expect.element(head).toHaveAttribute("href", RUN);
	await expect.element(head).toHaveAccessibleName("9c1f2e4");
	const toggle = table().getByRole("row").nth(1).getByRole("button");
	expect(toggle.element().getAttribute("aria-label")).toMatch(
		/^main, run 9c1f2e4, 8 reports, ubuntu-latest, macos-latest, magic, by GitHub Actions, Sep 1[34], 2026, \d\d:16$/,
	);
	expectNameStartsWithText(toggle.element());
});

// Kills a `+N` that counts the names sent rather than the run's, a hover that
// lists only the first, drops what the API left out, or that the row's link
// covers so the pointer never reaches it, a count shown for a run with one, and
// a row toggle whose name leaves the lists out.
test("testbeds, adapters, and authors show the first and a count whose hover lists them all", async () => {
	await page.viewport(1280, 720);
	mount([
		runFixture(0, {
			testbeds: ["t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8"],
			testbed_count: 9,
			adapters: ["json", "magic"] as never,
			adapter_count: 2,
			authors: [
				{ kind: "key", name: "ci-matrix" },
				{ kind: "user", name: "Courage" },
			] as never,
			author_count: 2,
		}),
		runFixture(1),
	]);
	await expect.poll(drawn).toEqual([2, 3]);

	expect(more(2)).toEqual([
		["+8", "t1, t2, t3, t4, t5, t6, t7, t8, and 1 more"],
		["+1", "json, magic"],
		["+1", "ci-matrix, Courage"],
	]);
	expect(more(3)).toEqual([]);
	expect(
		table().getByRole("button").first().element().getAttribute("aria-label"),
	).toContain(
		"t1, t2, t3, t4, t5, t6, t7, t8, and 1 more, json, magic, by ci-matrix, Courage, ",
	);
	// A row keeps its height however many it names.
	const rows = [
		...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]"),
	];
	expect(rows.map((row) => row.getBoundingClientRect().height)).toEqual([
		40, 40,
	]);
});

// Kills a run without a hash drawn blank, or without its version's name on a
// hover the pointer can reach.
test("a run without a hash reads as its version", async () => {
	await page.viewport(1280, 720);
	mount([runFixture(0, { hash: undefined, number: 252 })]);
	const head = () =>
		document.querySelector("tr[aria-rowindex='2'] .reports-hash");
	await expect.poll(() => head()?.textContent).toBe("v252");
	expect(hovered(head() as Element)).toBe("Version 252");
	expect(
		table().getByRole("button").element().getAttribute("aria-label"),
	).toMatch(/^main, run v252, /);
	await expect
		.element(table().getByRole("link", { name: "v252" }))
		.toHaveAttribute("href", RUN);
});

// Kills a narrow row that keeps every column, is shorter than a touch target,
// drops the run's report count or its caret, or that only its text toggles,
// and a head whose tap target is its text's line, reaches past its row, or
// covers the caret.
test("narrow rows fold into two lines that toggle the run anywhere, the head a touch target", async () => {
	await page.viewport(390, 844);
	mount([
		runFixture(0, { reports: 3 }),
		runFixture(1, {
			reports: 2,
			jobs: { queued: 0, running: 2, failed: 0, canceled: 0, since: NOW },
		}),
	]);
	await expect.poll(drawn).toEqual([2, 3]);
	for (const each of document.querySelectorAll<HTMLElement>(
		"tbody tr[aria-rowindex]",
	)) {
		const link = each.querySelector(".reports-hash a") as HTMLElement;
		const band = link.getBoundingClientRect();
		const box = each.getBoundingClientRect();
		expect(band.height).toBeGreaterThanOrEqual(44);
		expect(band.top).toBeGreaterThanOrEqual(box.top - 0.5);
		expect(band.bottom).toBeLessThanOrEqual(box.bottom + 0.5);
		for (const y of [band.top + 1, band.bottom - 1]) {
			expect(
				document.elementFromPoint(band.x + band.width / 2, y)?.closest("a"),
			).toBe(link);
		}
		const caret = (
			each.querySelector(".rn-caret") as Element
		).getBoundingClientRect();
		expect(
			document
				.elementFromPoint(caret.x + caret.width / 2, caret.y + caret.height / 2)
				?.closest("button"),
		).toBe(each.querySelector("button"));
	}
	const row = document.querySelector("tr[aria-rowindex='2']") as HTMLElement;
	await expect.poll(() => row.getBoundingClientRect().height).toBe(60);
	expect(document.querySelector("thead")?.checkVisibility()).toBe(false);
	const count = row.querySelector(".rn-n") as HTMLElement;
	expect(count.checkVisibility()).toBe(true);
	expect(count.textContent?.replace(/\s+/g, " ").trim()).toBe("· 3 reports");
	expect(count.querySelector(".rn-caret")?.checkVisibility()).toBe(true);
	const hash = row.querySelector(".reports-hash") as HTMLElement;
	expect(hash.getBoundingClientRect().bottom).toBeLessThanOrEqual(
		count.getBoundingClientRect().top + 1,
	);

	const box = row.getBoundingClientRect();
	const hit = document.elementFromPoint(box.right - 4, box.bottom - 4);
	expect(hit?.closest("button")).toBe(row.querySelector("button"));
});

/**
 * Asserts that a click on any drawn cell of `row` lands on its toggle, a
 * click just above or below the row does not, and a click on its head or any
 * other hover target lands on that.
 */
const expectRowToggles = (row: Element) => {
	const toggle = row.querySelector("button");
	const own = [...row.querySelectorAll("a, .rowlink-own")];
	const hit = (element: Element) => {
		const box = element.getBoundingClientRect();
		return document.elementFromPoint(
			box.x + box.width / 2,
			box.y + box.height / 2,
		);
	};
	let cells = 0;
	for (const cell of row.querySelectorAll("td")) {
		if (
			cell.getClientRects().length > 0 &&
			!own.some((target) => cell.contains(target))
		) {
			cells++;
			expect(hit(cell)?.closest("button")).toBe(toggle);
		}
	}
	expect(cells).toBeGreaterThan(1);
	const box = row.getBoundingClientRect();
	for (const y of [box.top - 2, box.bottom + 2]) {
		const outside = document.elementFromPoint(box.x + box.width / 2, y);
		expect(outside?.closest("button")).not.toBe(toggle);
	}
	for (const target of own) {
		expect(target.contains(hit(target))).toBe(true);
	}
};

// Kills a wide row only its branch toggles, one whose toggle covers the rows
// around it or its head, a head that links anything but its run, and a `+N`
// or a version a click on passes through to the toggle.
test("a wide row toggles from any cell, and its head opens the run", async () => {
	await page.viewport(1280, 720);
	mount([
		runFixture(0),
		runFixture(1, {
			testbeds: ["ubuntu-latest", "macos-latest"],
			testbed_count: 2,
		}),
		runFixture(2, { hash: undefined }),
	]);
	await expect.poll(drawn).toEqual([2, 3, 4]);
	const rows = [...document.querySelectorAll("tbody tr[aria-rowindex]")];
	for (const row of rows) {
		expectRowToggles(row);
	}
	expect(
		rows.map((row) =>
			row.querySelector(".reports-hash a")?.getAttribute("href"),
		),
	).toEqual([0, 1, 2].map((index) => RUN.replace(/0$/, String(index))));
});

/** Runs whose later rows hold longer names, more testbeds, and alerts, which an auto layout would widen its columns for. */
const varied = () =>
	Array.from({ length: 120 }, (_, index) =>
		runFixture(
			index,
			index < 40
				? {}
				: {
						branch: {
							uuid: "b",
							name: "feature/a-branch-with-a-rather-long-name",
							slug: "b",
							head: "h",
						} as never,
						testbeds: ["ubuntu-22.04-arm64-large-runner", "macos-latest"],
						testbed_count: 5,
						alerts: { total: 120, active: 64 },
						lines: 123_456,
						authors: [
							{ kind: "key", name: "a project key with a long name" },
						] as never,
					},
		),
	);

// Kills a table laid out by the rows drawn (an auto layout), whose columns
// shift as rows of other widths scroll into view, at desktop and at 768 px.
test("columns hold still as rows of other widths scroll into view", async () => {
	for (const width of [1280, 768]) {
		await page.viewport(width, 720);
		mount(varied());
		await expect.element(table()).toBeVisible();
		const before = columnEdges(table().element());
		expect(before.cells).toEqual(before.headers);
		await expect
			.poll(() => {
				window.scrollTo({
					top: document.body.scrollHeight,
					behavior: "instant",
				});
				return drawn().at(-1);
			})
			.toBe(121);
		const after = columnEdges(table().element());
		expect(after.headers).toEqual(before.headers);
		expect(after.cells).toEqual(before.headers);
		window.scrollTo({ top: 0, behavior: "instant" });
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a table that does not fold on a phone (an auto layout there), whose
// rows' cells follow the widest rows drawn beside them.
test("on a phone, each row folds on its own, whatever rows are drawn beside it", async () => {
	await page.viewport(390, 720);
	const firstRow = async (runs: JsonConsoleRunSummary[]) => {
		mount(runs);
		await expect.element(table()).toBeVisible();
		const { cells } = columnEdges(table().element());
		dispose?.();
		document.body.replaceChildren();
		return cells;
	};
	const runs = varied();
	expect(await firstRow([...runs.slice(0, 1), ...runs.slice(40, 60)])).toEqual(
		await firstRow(runs.slice(0, 20)),
	);
});

// Kills a first name that pushes its `+N` past the cell's edge, out of the
// pointer's reach (a name that does not give way), and an Alerts column that
// cuts off three-digit counts, at desktop and on a small laptop.
test("a long first name gives way to its +N, and three-digit alerts fit", async () => {
	for (const width of [1280, 1024]) {
		await page.viewport(width, 720);
		mount([
			runFixture(0, {
				testbeds: ["ubuntu-22.04-arm64-large-runner", "macos-latest"],
				testbed_count: 5,
				authors: [
					{ kind: "key", name: "a project key with a rather long name" },
					{ kind: "user", name: "Muriel Bagge" },
				] as never,
				author_count: 2,
				alerts: { total: 120, active: 64 },
			}),
		]);
		await expect.element(table()).toBeVisible();
		expect(more(2)).toEqual([
			["+4", "ubuntu-22.04-arm64-large-runner, macos-latest, and 3 more"],
			["+1", "a project key with a rather long name, Muriel Bagge"],
		]);
		const alerts = document.querySelector(
			"tr[aria-rowindex='2'] .reports-alerts",
		)?.parentElement as HTMLElement;
		expect(alerts.scrollWidth).toBeLessThanOrEqual(alerts.clientWidth);
		dispose?.();
		document.body.replaceChildren();
	}
});

const jobs = (fields: Partial<JsonConsoleRunJobs>) =>
	({
		queued: 0,
		running: 0,
		failed: 0,
		canceled: 0,
		...fields,
	}) as JsonConsoleRunJobs;

/** A run of two reports, both runner jobs in the states `fields` counts. */
const pending = (index: number, fields: Partial<JsonConsoleRunJobs>) =>
	runFixture(index, { reports: 2, jobs: jobs(fields) });

// Kills a run's status read from anything but its jobs, a time on a queued or
// canceled run or none on a running or failed one, a count drawn for a run
// nothing has landed for and a dash for one something has, and a toggle whose
// name leaves out a pending run's status.
test("a run's status column reads its jobs, its counts a dash until something lands", async () => {
	await page.viewport(1280, 720);
	mount([
		pending(0, { running: 2, since: NOW - 48_000 }),
		pending(1, { queued: 2, since: NOW - 5_000 }),
		pending(2, { failed: 1, canceled: 1 }),
		pending(3, { canceled: 2 }),
		runFixture(4, {
			reports: 3,
			jobs: jobs({ running: 1, since: NOW - 125_000 }),
		}),
		runFixture(5),
	]);
	await expect.element(table()).toBeVisible();
	const tail = (row: number) => cells(row).slice(6, 9);
	expect(tail(2)).toEqual(["-", "-", "Running 48s"]);
	expect(tail(3)).toEqual(["-", "-", "Queued"]);
	expect(tail(4)).toEqual(["-", "-", "Failed 2m 00s"]);
	expect(tail(5)).toEqual(["-", "-", "Canceled"]);
	expect(tail(6)).toEqual(["36", "0", "Running 2m"]);
	expect(tail(7)).toEqual(["36", "0", "2m 00s"]);
	await expect
		.element(table().getByRole("button", { name: /, Running 48s$/ }))
		.toBeVisible();
	expect(
		document
			.querySelector("tr[aria-rowindex='7'] button")
			?.getAttribute("aria-label"),
	).not.toMatch(/2m 00s/);
});

// Kills a queued pill without its open ring, a running dot that holds still,
// a failed pill drawn like the alert pill, and a canceled pill filled like the
// others.
test("each status draws its own mark and fill", async () => {
	await page.viewport(1280, 720);
	mount([
		pending(0, { queued: 2, since: NOW }),
		pending(1, { running: 2, since: NOW }),
		pending(2, { failed: 2 }),
		pending(3, { canceled: 2 }),
		runFixture(4, { alerts: { total: 1, active: 1 } }),
	]);
	await expect.element(table()).toBeVisible();
	const pill = (status: string) =>
		document.querySelector(`.rn-st[data-status="${status}"]`) as HTMLElement;
	const style = (element: Element | null) =>
		getComputedStyle(element as Element);
	const ring = style(pill("queued").querySelector(".rn-ring"));
	expect(ring.borderTopWidth).not.toBe("0px");
	expect(ring.backgroundColor).toBe("rgba(0, 0, 0, 0)");
	expect(style(pill("running").querySelector(".rn-pulse")).animationName).toBe(
		"rn-pulse",
	);
	expect(pill("failed").querySelector("svg")).not.toBeNull();
	const alert = style(document.querySelector(".reports-active"));
	const failed = style(pill("failed"));
	expect(failed.backgroundColor).not.toBe(alert.backgroundColor);
	expect(failed.color).not.toBe(alert.color);
	expect(style(pill("canceled")).backgroundColor).toBe("rgba(0, 0, 0, 0)");
	expect(pill("canceled").querySelector("svg")).not.toBeNull();
});

// Kills a phone row that hides a pending run's status, or shows a finished
// run's time in place of its alerts.
test("on a phone a pending run shows its status where a finished run shows its alerts", async () => {
	await page.viewport(390, 844);
	mount([
		pending(0, { running: 2, since: NOW - 48_000 }),
		runFixture(1, { alerts: { total: 2, active: 1 } }),
	]);
	await expect.element(table()).toBeVisible();
	const shown = (row: number, selector: string) =>
		(
			document.querySelector(
				`tr[aria-rowindex="${row}"] ${selector}`,
			) as HTMLElement
		).checkVisibility();
	expect(shown(2, ".rn-st")).toBe(true);
	expect(shown(3, ".reports-alerts")).toBe(true);
	expect(shown(3, "td:nth-child(9)")).toBe(false);
});

/** The fixed-length columns, which hold their content whole from 768 px up (rulings 409 and 410). */
const FIXED = ["When", "Head", "Reports", "Lines", "Alerts", "Status"];
const statuses = () =>
	[...document.querySelectorAll("tbody .rn-st")].map(
		(pill) => pill.textContent,
	);
const runRows = () => [...document.querySelectorAll("tbody tr[aria-rowindex]")];

// Kills a list below a laptop that cuts a fixed-length cell (its time, head,
// counts, alerts, or status) or a header rather than scroll, and a Status
// column too narrow for its longest pills or given back its end padding
// (ruling 410: from 768 to 1024 px the list keeps one layout).
test("at 768 px the list scrolls sideways, its fixed-length cells and headers whole", async () => {
	await page.viewport(768, 720);
	mount(likelyRuns(NOW));
	await expect.element(table()).toBeVisible();
	await document.fonts.ready;
	expect(table().element().getBoundingClientRect().width).toBeGreaterThan(
		document.documentElement.clientWidth,
	);
	expect(statuses()).toEqual([
		"Running 59m",
		"Failed 59m 59s",
		"Failed 25h 59m",
	]);
	expect(runRows().flatMap((row) => cutCells(row, FIXED))).toEqual([]);
	expect(cutHeaders(table().element())).toEqual([]);
});

// Kills a list that scrolls sideways on a small laptop, and one that cuts a
// fixed-length cell (its time, head, counts, alerts, or status) or a header
// there; the branch, testbed, adapter, and author take the shortfall (ruling
// 409).
test("at 1024 px the list fits, its fixed-length cells and headers whole", async () => {
	await page.viewport(1024, 720);
	mount(likelyRuns(NOW));
	await expect.element(table()).toBeVisible();
	await document.fonts.ready;
	expect(table().element().getBoundingClientRect().width).toBeLessThanOrEqual(
		document.documentElement.clientWidth,
	);
	expect(statuses()).toEqual([
		"Running 59m",
		"Failed 59m 59s",
		"Failed 25h 59m",
	]);
	expect(runRows().flatMap((row) => cutCells(row, FIXED))).toEqual([]);
	expect(cutHeaders(table().element())).toEqual([]);
});

const START = runReportFixture(0).start_time;
const job = (fields: Partial<JsonConsoleJob>) =>
	({ status: "queued", created: START, ...fields }) as JsonConsoleJob;
const toggleOf = (name: RegExp) => table().getByRole("button", { name });
/** Waits until no row holds a place: every opened run's rows are read and their code is in. */
const childrenDrawn = () =>
	expect
		.poll(() => document.querySelectorAll('tbody tr[aria-busy="true"]').length)
		.toBe(0);
/** A click on `cell`, a cell of a run's row away from its toggle's text. */
const clickCell = (row: number, cell: number) =>
	userEvent.click(
		document.querySelector(
			`tr[aria-rowindex="${row}"] td:nth-child(${cell})`,
		) as HTMLElement,
		{ force: true },
	);

// Kills a toggle that is not a button with its state, a click away from its
// text or a key on it that does nothing, and a run that opens another's rows.
test("a run opens and closes by a click anywhere on its row and by keyboard on its toggle", async () => {
	await page.viewport(1280, 720);
	const [first, second] = [
		runFixture(0, { reports: 2 }),
		runFixture(1, { reports: 1 }),
	];
	mount([first, second], 2, {
		reads: { [first.uuid]: readOf(2), [second.uuid]: readOf(1) },
	});
	const toggle = toggleOf(/^main, run 9c1f2e4, 2 reports, /);
	await expect.element(toggle).toHaveAttribute("aria-expanded", "false");

	await clickCell(2, 7);
	await expect.element(toggle).toHaveAttribute("aria-expanded", "true");
	expect(drawn()).toEqual([2, 3, 4, 5]);
	expect(toggled).toEqual([first.uuid]);
	await clickCell(2, 1);
	await expect.element(toggle).toHaveAttribute("aria-expanded", "false");
	expect(drawn()).toEqual([2, 3]);

	(toggle.element() as HTMLElement).focus();
	await userEvent.keyboard("{Enter}");
	await expect.element(toggle).toHaveAttribute("aria-expanded", "true");
	await userEvent.keyboard(" ");
	await expect.element(toggle).toHaveAttribute("aria-expanded", "false");
	expect(toggled).toEqual(Array(4).fill(first.uuid));
	await expect
		.element(toggleOf(/^main, run 9c1f2e4, 1 report, /))
		.toHaveAttribute("aria-expanded", "false");
});

// Kills a report row that reads the wrong field or puts it under another
// column, a time shown for a job no runner took, counts drawn for a job
// still pending, a status read from anything but its job, and a link to
// anything but the report, or whose name leaves its job's status out.
test("an opened run's reports read in the run's columns, a pending job's status where its counts will land", async () => {
	await page.viewport(1280, 720);
	const run = runFixture(0, { reports: 3 });
	const read = {
		data: runPageFixture(
			[
				runReportFixture(0),
				runReportFixture(1, {
					testbed: {
						uuid: "m",
						name: "macos-latest",
						slug: "macos-latest",
					} as never,
					adapter: "magic" as never,
					job: job({
						status: "running" as JsonConsoleJob["status"],
						started: NOW - 48_000,
					}),
					author: { kind: "key", name: "GitHub Actions" } as never,
				}),
				runReportFixture(2, { job: job({}), author: undefined }),
			],
			{ total: 3 },
		),
	};
	mount([run, runFixture(1)], 2, {
		opened: [run.uuid],
		reads: { [run.uuid]: read },
	});
	await expect.poll(drawn).toEqual([2, 3, 4, 5, 6]);
	await childrenDrawn();
	expect(cells(3)).toEqual([
		expect.stringMatching(/^\d\d:\d\d:00$/),
		"1",
		"",
		"",
		"ubuntu-latest",
		"json",
		"36 lines",
		"0",
		"2m 00s",
		"Muriel Bagge",
	]);
	expect(cells(4)).toEqual([
		expect.stringMatching(/^\d\d:\d\d:01$/),
		"2",
		"",
		"",
		"macos-latest",
		"magic",
		"-",
		"-",
		"Running 48s",
		"GitHub Actions",
	]);
	expect(cells(5)).toEqual([
		"",
		"3",
		"",
		"",
		"ubuntu-latest",
		"json",
		"-",
		"-",
		"Queued",
		"",
	]);
	const report = (row: number) =>
		table()
			.getByRole("row")
			.nth(row - 1)
			.getByRole("link");
	await expect
		.element(report(4))
		.toHaveAttribute("href", `${REPORTS}/${runReportFixture(1).uuid}`);
	expect(report(4).element().getAttribute("aria-label")).toMatch(
		/^macos-latest, report 2, started \d\d:\d\d:01, Running 48s$/,
	);
	expect(report(5).element().getAttribute("aria-label")).toBe(
		"ubuntu-latest, report 3, Queued",
	);
	expectNameStartsWithText(report(3).element());
	for (const row of [3, 4, 5]) {
		expectRowOpens(
			document.querySelector(`tr[aria-rowindex="${row}"]`) as Element,
			`${REPORTS}/${runReportFixture(row - 3).uuid}`,
		);
	}
});

// Kills an opened run that lists more than its first eight reports, an All
// row missing, worded otherwise, or counting anything but the run's reports,
// and one that opens anything but the run's page, or only from its text.
test("an opened run lists its first eight reports, then a row that opens the run's page for all of them", async () => {
	await page.viewport(1280, 720);
	const run = runFixture(0, { reports: 12 });
	mount([run, runFixture(1)], 2, {
		opened: [run.uuid],
		reads: { [run.uuid]: readOf(8, 13) },
	});
	await expect.poll(() => drawn().length).toBe(11);
	await childrenDrawn();
	expect(cells(10)[1]).toBe("8");
	const all = table().getByRole("link", {
		name: "All 13 reports",
		exact: true,
	});
	await expect.element(all).toHaveAttribute("href", RUN);
	expectRowOpens(
		document.querySelector('tr[aria-rowindex="11"]') as Element,
		RUN,
	);
	expect(cells(12)[1]).toBe("main");
});

// Kills runs that cannot be open together, and an opened run's rows drawn
// anywhere but under it.
test("many runs open at once, each with its reports under it", async () => {
	await page.viewport(1280, 720);
	const runs = [0, 1, 2].map((index) => runFixture(index, { reports: 2 }));
	const [first, , third] = runs as [
		JsonConsoleRunSummary,
		JsonConsoleRunSummary,
		JsonConsoleRunSummary,
	];
	mount(runs, 3, {
		opened: [first.uuid, third.uuid],
		reads: { [first.uuid]: readOf(2), [third.uuid]: readOf(2) },
	});
	await expect.poll(() => drawn().length).toBe(7);
	await childrenDrawn();
	expect(
		[...document.querySelectorAll("tbody tr[aria-rowindex]")].map(
			(row) =>
				row.querySelector(".reports-hash a")?.textContent ??
				row.querySelector(".rn-ord")?.textContent,
		),
	).toEqual(["9c1f2e4", "1", "2", "9c1f2e4", "9c1f2e4", "1", "2"]);
	expect(
		[...document.querySelectorAll("tbody .rn-toggle")].map((toggle) =>
			toggle.getAttribute("aria-expanded"),
		),
	).toEqual(["true", "false", "true"]);
});

/** Where each run row's top edge sits. */
const runTops = () =>
	[...document.querySelectorAll("tbody tr:has(.rn-toggle)")].map(
		(row) => row.getBoundingClientRect().top,
	);

// Kills an opened run whose rows wait for its reports to hold their place, so
// their arrival pushes the runs below down, a placeholder of another height,
// and placeholders for more reports than the run lists.
test("an opened run holds its reports' rows until they arrive, so their arrival moves nothing", async () => {
	for (const [width, height] of [
		[1280, 40],
		[390, 60],
	] as const) {
		await page.viewport(width, 844);
		const [few, many] = [
			runFixture(0, { reports: 3 }),
			runFixture(1, { reports: 12 }),
		];
		mount([few, many, runFixture(2)], 3, { opened: [few.uuid, many.uuid] });
		await expect.poll(() => drawn().length).toBe(3 + 3 + 9);
		await expect
			.element(table().getByRole("link", { name: "All 12 reports" }))
			.toBeVisible();
		const before = runTops();
		const heights = [
			...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]"),
		].map((row) => row.getBoundingClientRect().height);
		expect(new Set(heights)).toEqual(new Set([height]));
		expect(
			document.querySelectorAll('tbody tr[aria-busy="true"] .ui-skeleton'),
		).toHaveLength(3 + 8);

		setReads({ [few.uuid]: readOf(3), [many.uuid]: readOf(8, 12) });
		await childrenDrawn();
		expect(runTops()).toEqual(before);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a failed read drawn as rows that never fill, more than one row, and
// a Retry that asks again for another run's reports, or for none.
test("an opened run whose reports failed shows one row whose Retry asks again", async () => {
	await page.viewport(1280, 720);
	const run = runFixture(0, { reports: 5 });
	mount([run, runFixture(1)], 2, {
		opened: [run.uuid],
		reads: { [run.uuid]: { isError: true } },
	});
	const alert = table().getByRole("alert");
	await expect.element(alert).toHaveTextContent("These reports did not load");
	expect(drawn()).toEqual([2, 3, 4]);
	await alert.getByRole("button", { name: "Retry" }).click();
	expect(retried).toEqual([run.uuid]);
	const [row] = [
		...document.querySelectorAll<HTMLElement>('tr[aria-rowindex="3"]'),
	];
	expect(row?.getBoundingClientRect().height).toBe(40);
});

// Kills a row count or row numbers that leave an opened run's rows out, and
// a range on screen counted in rows rather than runs, which would ask for the
// next batch of runs too soon while one is open.
test("row numbers and the count take in opened runs' rows, and the range on screen counts runs", async () => {
	await page.viewport(1280, 720);
	const runs = Array.from({ length: 60 }, (_, index) =>
		runFixture(index, { reports: index === 0 ? 4 : 1 }),
	);
	mount(runs, 100, {
		opened: [runs[0]?.uuid ?? ""],
		reads: { [runs[0]?.uuid ?? ""]: readOf(4) },
	});
	await expect.element(table()).toHaveAttribute("aria-rowcount", "105");
	await childrenDrawn();
	await expect.poll(() => drawn().slice(0, 7)).toEqual([2, 3, 4, 5, 6, 7, 8]);
	const onScreen = ranges.at(-1);
	expect(onScreen?.start).toBe(0);
	// About 16 rows fit, four of them the opened run's.
	expect(onScreen?.end).toBeGreaterThanOrEqual(11);
	expect(onScreen?.end).toBeLessThanOrEqual(15);
});

// Kills a caret that does not turn as its run opens, or stays turned once it closes.
test("a run's caret turns as it opens", async () => {
	await page.viewport(1280, 720);
	const run = runFixture(0, { reports: 2 });
	mount([run], 1, { reads: { [run.uuid]: readOf(2) } });
	const caret = () =>
		getComputedStyle(document.querySelector(".rn-caret") as Element).transform;
	await expect.element(table()).toBeVisible();
	const closed = caret();
	await clickCell(2, 7);
	await expect.poll(caret).not.toBe(closed);
	await clickCell(2, 7);
	await expect.poll(caret).toBe(closed);
});

// Kills a run whose reports are asked for only once clicked, or as the page
// moves under a still pointer, and a report row whose page starts so, or never.
test("the pointer moving onto a run, or focus on its toggle, starts its reports; moving onto a report starts its page", async () => {
	await page.viewport(1280, 720);
	const [first, second] = [
		runFixture(0, { reports: 1 }),
		runFixture(1, { reports: 1 }),
	];
	mount([first, second], 2, {
		opened: [first.uuid],
		reads: { [first.uuid]: readOf(1) },
	});
	const [opened, closed] = document.querySelectorAll<HTMLElement>(".rn-toggle");
	closed?.dispatchEvent(new PointerEvent("pointermove", { bubbles: true }));
	expect(intents).toEqual([]);
	await userEvent.hover(closed as HTMLElement);
	expect(intents).toEqual([`run ${second.uuid}`]);
	opened?.focus();
	expect(intents.at(-1)).toBe(`run ${first.uuid}`);

	await childrenDrawn();
	const link = document.querySelector('tr[aria-rowindex="3"] a') as Element;
	const move = (movementX: number) =>
		link.dispatchEvent(
			new PointerEvent("pointermove", { bubbles: true, movementX }),
		);
	intents.length = 0;
	move(0);
	expect(intents).toEqual([]);
	move(3);
	expect(intents).toEqual([`report ${runReportFixture(0).uuid}`]);
});

// Kills a head or a row for the rest of a run's reports whose run page waits
// for the click, or starts as the page moves under a still pointer, from a
// move, a focus, or a touch, and a run whose toggle starts its page.
test("a run's head and its row for the rest start its page on a real move, focus, or touch, and its toggle does not", async () => {
	await page.viewport(1280, 720);
	const run = runFixture(0, { reports: 9 });
	mount([run], 1, {
		opened: [run.uuid],
		reads: { [run.uuid]: readOf(8, 9) },
	});
	await childrenDrawn();
	const toggle = document.querySelector(".rn-toggle") as HTMLElement;
	const head = document.querySelector(".reports-hash a") as HTMLElement;
	const all = table()
		.getByRole("link", { name: "All 9 reports" })
		.element() as HTMLElement;
	const move = (target: Element, movementX: number) =>
		target.dispatchEvent(
			new PointerEvent("pointermove", { bubbles: true, movementX }),
		);
	const touch = (target: Element) =>
		target.dispatchEvent(new Event("touchstart", { bubbles: true }));
	const intent = `page ${run.uuid}`;
	for (const target of [head, all]) {
		intents.length = 0;
		move(target, 0);
		expect(intents).toEqual([]);
		move(target, 3);
		expect(intents).toEqual([intent]);
		target.focus();
		expect(intents).toEqual([intent, intent]);
		target.blur();
		touch(target);
		expect(intents).toEqual([intent, intent, intent]);
	}
	intents.length = 0;
	move(toggle, 3);
	toggle.focus();
	expect(intents.filter((each) => each === intent)).toEqual([]);
});

// Kills an opened run's report whose fixed-length cells (its time, counts,
// alerts, or status) a list below a laptop or on a small laptop cuts.
test("an opened run's likely reports keep their fixed-length cells whole at 768 and 1024 px", async () => {
	for (const width of [768, 1024]) {
		await page.viewport(width, 720);
		const run = runFixture(0, { reports: 2 });
		mount([run], 1, {
			opened: [run.uuid],
			reads: {
				[run.uuid]: {
					data: runPageFixture([
						likelyReport(0, { start_time: START + 37 * 60_000 }),
						likelyReport(1, {
							job: job({
								status: "failed" as JsonConsoleJob["status"],
								started: START,
								completed: START + 59 * 60_000 + 59_000,
							}),
						}),
					]),
				},
			},
		});
		await expect.element(table()).toBeVisible();
		await childrenDrawn();
		await document.fonts.ready;
		expect(statuses()).toEqual(["Failed 59m 59s"]);
		expect(runRows().flatMap((row) => cutCells(row, FIXED))).toEqual([]);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills an opened run's rows that keep every column on a phone, put the order
// anywhere but beside two lines, or a status or alerts anywhere but on the right.
test("on a phone an opened run's reports fold, the order beside two lines and the status or alerts on the right", async () => {
	await page.viewport(390, 844);
	const run = runFixture(0, { reports: 2 });
	mount([run], 1, {
		opened: [run.uuid],
		reads: {
			[run.uuid]: {
				data: runPageFixture([
					runReportFixture(0, { alerts: { total: 2, active: 1 } }),
					runReportFixture(1, { job: job({}) }),
				]),
			},
		},
	});
	await expect.poll(() => drawn().length).toBe(3);
	await childrenDrawn();
	for (const [index, right] of [
		[3, ".reports-alerts"],
		[4, ".rn-st"],
	] as const) {
		const row = document.querySelector(
			`tr[aria-rowindex="${index}"]`,
		) as HTMLElement;
		const box = (selector: string) =>
			(row.querySelector(selector) as HTMLElement).getBoundingClientRect();
		expect(row.getBoundingClientRect().height).toBe(60);
		const order = box(".rn-ord");
		const testbed = box("a");
		expect(order.right).toBeLessThanOrEqual(testbed.left);
		expect(box(right).left).toBeGreaterThan(testbed.right);
		expect(box(right).right).toBeGreaterThan(
			row.getBoundingClientRect().right - 40,
		);
		expect(row.querySelector("td:nth-child(6)")?.checkVisibility()).toBe(false);
	}
	const time = document.querySelector(
		'tr[aria-rowindex="3"] time',
	) as HTMLElement;
	expect(time.getBoundingClientRect().top).toBeGreaterThanOrEqual(
		(
			document.querySelector('tr[aria-rowindex="3"] a') as HTMLElement
		).getBoundingClientRect().bottom - 1,
	);
});
