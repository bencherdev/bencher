import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./reports.css";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { JsonReport } from "../../types/bencher";
import ReportsTable from "./ReportsTable";
import type { Range } from "./rows";
import { reportFixture } from "./testing";

const NOW = Date.parse("2026-09-14T00:00:00Z");

let dispose: (() => void) | undefined;
const ranges: Range[] = [];

const mount = (reports: JsonReport[], total = reports.length) => {
	ranges.length = 0;
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<ReportsTable
				slug="hashbrown"
				reports={reports}
				total={total}
				now={NOW}
				onRange={(range) => ranges.push(range)}
			/>
		),
		root,
	);
};

const table = () => page.getByRole("table", { name: "Reports, newest first" });
const drawn = () =>
	[...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]")].map(
		(row) => Number(row.getAttribute("aria-rowindex")),
	);
const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));

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
		Array.from({ length: 500 }, (_, index) => reportFixture(index)),
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
	await frame();
	await frame();
	await expect.poll(() => drawn().includes(302)).toBe(true);
	expect(drawn()).not.toContain(2);
	expect(drawn().length).toBeLessThan(40);
	// The rows on screen, not those drawn around them: 300 and the 18 below it.
	const onScreen = ranges.at(-1);
	expect(onScreen?.start).toBeGreaterThanOrEqual(299);
	expect(onScreen?.start).toBeLessThanOrEqual(300);
	expect((onScreen?.end ?? 0) - (onScreen?.start ?? 0)).toBeLessThanOrEqual(20);

	// A second scroll, once nothing else can have moved the rows.
	const top = window.scrollY;
	window.scrollTo({ top: top - 200 * 40, behavior: "instant" });
	await expect.poll(() => drawn().includes(102)).toBe(true);
	expect(drawn()).not.toContain(302);
});

// Kills cells that read the wrong field, a key run named as a person, and
// an alerts cell that drops the active count or the total.
test("a row reads its report", async () => {
	await page.viewport(1280, 720);
	mount([
		reportFixture(0, {
			user: undefined,
			project_key: { uuid: "key", name: "GitHub Actions" },
			testbed: { name: "macos-latest", slug: "macos-latest" } as never,
			adapter: "magic" as never,
			end_time: new Date(
				Date.parse("2026-09-13T21:46:00Z") + 252_000,
			).toISOString(),
			counts: {
				results: [{ benchmarks: 2, measures: 2, lines: 4 }],
				alerts: { total: 2, active: 0 },
			},
		}),
		reportFixture(1, {
			counts: {
				results: [{ benchmarks: 18, measures: 2, lines: 36 }],
				alerts: { total: 3, active: 1 },
			},
		}),
		reportFixture(2),
	]);

	const rows = table().getByRole("row");
	await expect
		.element(rows.nth(1).getByRole("cell").nth(0))
		.toHaveTextContent("2h ago");
	const cells = (row: number) =>
		[...document.querySelectorAll(`tr[aria-rowindex="${row}"] td`)].map(
			(cell) => cell.textContent?.trim(),
		);
	expect(cells(2)).toEqual([
		"2h ago",
		"main",
		"9c1f2e4",
		"macos-latest",
		"magic",
		"4",
		"2 total",
		"4m 12s",
		"GitHub Actions",
	]);
	expect(cells(3)).toEqual([
		"3h ago",
		"main",
		"9c1f2e4",
		"ubuntu-latest",
		"json",
		"36",
		"1 active 3 total",
		"2m 00s",
		"Muriel Bagge",
	]);
	expect(cells(4)[6]).toBe("0");
	await expect
		.element(rows.nth(1).getByRole("img", { name: "Project key" }))
		.toBeVisible();
	await expect
		.element(rows.nth(2).getByRole("img", { name: "User" }))
		.toBeVisible();

	const link = rows.nth(1).getByRole("link");
	await expect
		.element(link)
		.toHaveAttribute(
			"href",
			"/next/console/projects/hashbrown/reports/00000000-0000-4000-8000-000000000000",
		);
	expect(link.element().getAttribute("aria-label")).toMatch(
		/^Report on main, macos-latest, magic, Sep 1[34], 2026, \d\d:46, 9c1f2e4$/,
	);
});

// Kills a narrow row that keeps every column, is shorter than a touch
// target, or that only its text opens.
test("narrow rows fold into two lines that open the report anywhere", async () => {
	await page.viewport(390, 844);
	mount([reportFixture(0)]);
	const row = document.querySelector("tr[aria-rowindex='2']") as HTMLElement;
	await expect.poll(() => row.getBoundingClientRect().height).toBe(60);
	expect(document.querySelector("thead")?.checkVisibility()).toBe(false);

	const box = row.getBoundingClientRect();
	const hit = document.elementFromPoint(box.right - 4, box.bottom - 4);
	expect(hit?.closest("a")?.getAttribute("href")).toBe(
		"/next/console/projects/hashbrown/reports/00000000-0000-4000-8000-000000000000",
	);
});
