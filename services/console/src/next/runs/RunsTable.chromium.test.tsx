import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type {
	JsonConsoleRunJobs,
	JsonConsoleRunSummary,
} from "../../types/bencher";
import type { Range } from "../reports/rows";
import {
	columnEdges,
	expectNameStartsWithText,
	expectRowOpens,
} from "../ui/testing";
import RunsTable from "./RunsTable";
import { cutCells, cutHeaders, likelyRuns, runFixture } from "./testing";

const NOW = Date.parse("2026-09-14T00:00:00Z");
const RUN =
	"/next/console/projects/hashbrown/runs/00000000-0000-4000-8000-000000000000";

let dispose: (() => void) | undefined;
const ranges: Range[] = [];

const mount = (runs: JsonConsoleRunSummary[], total = runs.length) => {
	ranges.length = 0;
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<RunsTable
				slug="hashbrown"
				runs={runs}
				total={total}
				now={NOW}
				onRange={(range) => ranges.push(range)}
			/>
		),
		root,
	);
};

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
// drops the active count or the total, and a link to anything but the run.
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

	const link = table().getByRole("row").nth(1).getByRole("link");
	await expect.element(link).toHaveAttribute("href", RUN);
	expect(link.element().getAttribute("aria-label")).toMatch(
		/^main, run 9c1f2e4, 8 reports, ubuntu-latest, macos-latest, magic, by GitHub Actions, Sep 1[34], 2026, \d\d:16$/,
	);
	expectNameStartsWithText(link.element());
});

// Kills a `+N` that counts the names sent rather than the run's, a hover that
// lists only the first, drops what the API left out, or that the row's link
// covers so the pointer never reaches it, a count shown for a run with one, and
// a row link whose name leaves the lists out.
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
		table().getByRole("link").first().element().getAttribute("aria-label"),
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
		table().getByRole("link").element().getAttribute("aria-label"),
	).toMatch(/^main, run v252, /);
});

// Kills a narrow row that keeps every column, is shorter than a touch target,
// drops the run's report count, or that only its text opens.
test("narrow rows fold into two lines that open the run anywhere", async () => {
	await page.viewport(390, 844);
	mount([runFixture(0, { reports: 3 })]);
	const row = document.querySelector("tr[aria-rowindex='2']") as HTMLElement;
	await expect.poll(() => row.getBoundingClientRect().height).toBe(60);
	expect(document.querySelector("thead")?.checkVisibility()).toBe(false);
	const count = row.querySelector(".rn-n") as HTMLElement;
	expect(count.checkVisibility()).toBe(true);
	expect(count.textContent?.replace(/\s+/g, " ").trim()).toBe("· 3 reports");
	const hash = row.querySelector(".reports-hash") as HTMLElement;
	expect(hash.getBoundingClientRect().bottom).toBeLessThanOrEqual(
		count.getBoundingClientRect().top + 1,
	);

	const box = row.getBoundingClientRect();
	const hit = document.elementFromPoint(box.right - 4, box.bottom - 4);
	expect(hit?.closest("a")?.getAttribute("href")).toBe(RUN);
});

// Kills a wide row only its branch opens, one whose link covers the rows
// around it, and a `+N` or a version a click on passes through to the run.
test("a wide row opens its run from any cell", async () => {
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
	for (const row of document.querySelectorAll("tbody tr[aria-rowindex]")) {
		expectRowOpens(
			row,
			row.querySelector("a")?.getAttribute("href") ?? "no link",
		);
	}
	const hrefs = [...document.querySelectorAll("tbody a")].map((link) =>
		link.getAttribute("href"),
	);
	expect(new Set(hrefs).size).toBe(3);
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
// nothing has landed for and a dash for one something has, and a link whose
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
		.element(table().getByRole("link", { name: /, Running 48s$/ }))
		.toBeVisible();
	expect(
		document
			.querySelector("tr[aria-rowindex='7'] a")
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
