import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "./report.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { type ComponentProps, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import LineTable from "./LineTable";
import { type ReportLine, groupsOf, linesOf, plotOf } from "./lines";
import { slotsOf } from "./slots";
import { longReport } from "./testing";

// One frame at 120 Hz.
const FRAME_MS = 8.3;
const RUNS = 9;

let dispose: (() => void) | undefined;

beforeEach(async () => {
	await page.viewport(1280, 800);
	document.documentElement.setAttribute(THEME_ATTRIBUTE, "dark");
	window.scrollTo(0, 0);
});

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (
	count: number,
	onNearEnd = () => {},
	aside?: ComponentProps<typeof LineTable>["aside"],
) => {
	const report = longReport(count);
	const lines = linesOf(report);
	const slots = slotsOf(groupsOf(report, "benchmark"), lines);
	const [expanded, setExpanded] = createSignal<ReadonlySet<string>>(new Set());
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<LineTable
				label="Lines"
				slots={slots}
				rows={slots.length}
				narrow={false}
				metrics={false}
				history="History, 4w"
				expanded={expanded()}
				onExpand={(line: ReportLine, open: boolean) => {
					const next = new Set(expanded());
					if (open) {
						next.add(line.key);
					} else {
						next.delete(line.key);
					}
					setExpanded(next);
				}}
				plot={(line) => ({
					data: plotOf(line, "main", "ubuntu-latest"),
					note: "4w, ending at this report",
					reportHref: (uuid) => `/reports/${uuid}`,
				})}
				onNearEnd={onNearEnd}
				{...(aside ? { aside } : {})}
			/>
		),
		root,
	);
	return { lines };
};

// The page scrolls smoothly; a test jumps.
const bottom = () =>
	window.scrollTo({
		top: document.documentElement.scrollHeight,
		behavior: "instant",
	});
const frame = () =>
	new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
const drawnRows = () =>
	document.querySelectorAll("tbody tr[aria-rowindex]").length;

// Kills a list that draws every row it holds, and one that does not follow the scroll.
test("draws only the rows on screen, and follows the scroll", async () => {
	mount(500);
	await expect
		.element(page.getByRole("button", { name: /^Expand / }).first())
		.toBeVisible();
	await frame();
	expect(drawnRows()).toBeGreaterThan(15);
	expect(drawnRows()).toBeLessThan(45);
	await expect
		.element(page.getByRole("button", { name: "Expand bench-49 n=9 Latency" }))
		.not.toBeInTheDocument();

	bottom();
	await expect
		.element(page.getByRole("button", { name: "Expand bench-49 n=9 Latency" }))
		.toBeInTheDocument();
	expect(drawnRows()).toBeLessThan(45);
	await expect
		.element(page.getByRole("button", { name: "Expand bench-0 n=0 Latency" }))
		.not.toBeInTheDocument();
});

// Kills a next batch asked for before the last rows come near, or never.
test("asks for more once the last rows loaded come near the screen", async () => {
	const onNearEnd = vi.fn();
	mount(200, onNearEnd);
	await expect
		.element(page.getByRole("button", { name: /^Expand / }).first())
		.toBeVisible();
	await frame();
	expect(onNearEnd).not.toHaveBeenCalled();
	bottom();
	await expect.poll(() => onNearEnd.mock.calls.length).toBeGreaterThan(0);
});

// Kills a hover that does not start the plot's code, a plot mounted in the
// frame that opens the row, and space that is not reserved before it.
test("an expanded row reserves its space at once and draws its plot after that frame paints", async () => {
	mount(20);
	const twist = page.getByRole("button", {
		name: "Expand bench-0 n=0 Latency",
	});
	const plotCode = () =>
		performance
			.getEntriesByType("resource")
			.some((entry) => new URL(entry.name).pathname.includes("/RowPlot"));
	expect(plotCode()).toBe(false);
	// Hovering loads the plot's code, so the click below can draw it at once if it does not wait.
	await twist.hover();
	await expect.poll(plotCode).toBe(true);
	await import("./RowPlot");
	await frame();
	const button = twist.element() as HTMLElement;
	const row = button.closest("tr") as HTMLElement;
	button.click();
	const space = row.nextElementSibling as HTMLElement;
	expect(space.getBoundingClientRect().height).toBe(330);
	expect(space.querySelector(".pl")).toBeNull();
	await expect
		.element(
			page
				.getByRole("region", { name: "bench-0 n=0 Latency, full plot" })
				.getByRole("button", { name: /^Hide / }),
		)
		.toBeVisible();
});

// Kills offsets that leave out an open row's plot, so the list shrinks once the row scrolls away.
test("an open row keeps its height once it scrolls out of view", async () => {
	mount(200);
	await page
		.getByRole("button", { name: "Expand bench-0 n=0 Latency" })
		.click();
	await expect
		.element(
			page.getByRole("region", { name: "bench-0 n=0 Latency, full plot" }),
		)
		.toBeInTheDocument();
	const height = () =>
		page.getByRole("table").element().getBoundingClientRect().height;
	const open = height();
	bottom();
	await expect
		.element(page.getByRole("button", { name: "Expand bench-19 n=9 Latency" }))
		.toBeInTheDocument();
	expect(height()).toBe(open);
});

const median = (values: number[]) =>
	[...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ??
	Number.NaN;

// Kills an expand or a key toggle that takes more than one 120 Hz frame of main thread work.
test("expanding a row and toggling its line in the key each fit in a frame", async () => {
	await import("./RowPlot");
	mount(60);
	await expect
		.element(page.getByRole("button", { name: /^Expand / }).first())
		.toBeVisible();
	const expands: number[] = [];
	const toggles: number[] = [];
	for (let run = 0; run < RUNS; run++) {
		const twist = page
			.getByRole("button", { name: /^Expand bench-0 n=0 Latency$/ })
			.element() as HTMLElement;
		const start = performance.now();
		twist.click();
		document.body.getBoundingClientRect();
		expands.push(performance.now() - start);

		const hide = page
			.getByRole("region", { name: "bench-0 n=0 Latency, full plot" })
			.getByRole("button", { name: /^Hide / });
		await expect.element(hide).toBeVisible();
		await frame();
		const toggle = performance.now();
		(hide.element() as HTMLElement).click();
		document.body.getBoundingClientRect();
		toggles.push(performance.now() - toggle);

		(
			page
				.getByRole("button", { name: "Collapse bench-0 n=0 Latency" })
				.element() as HTMLElement
		).click();
		await frame();
	}
	expect(median(expands)).toBeLessThan(FRAME_MS);
	expect(median(toggles)).toBeLessThan(FRAME_MS);
});

// Kills a group header that drops its name or its counts.
test("a group's header names it and counts its variants and lines", async () => {
	mount(12);
	await expect
		.element(page.getByRole("row").nth(1))
		.toHaveTextContent("bench-0 · 10 variants, 10 lines");
});

// Kills row numbers that skip an open row's plot, or a count that leaves it out.
test("numbers every row for a screen reader, an open line taking two", async () => {
	mount(3);
	await page
		.getByRole("button", { name: "Expand bench-0 n=1 Latency" })
		.click();
	await expect
		.element(
			page.getByRole("region", { name: "bench-0 n=1 Latency, full plot" }),
		)
		.toBeInTheDocument();
	const numbers = [
		...document.querySelectorAll<HTMLElement>("tr[aria-rowindex]"),
	].map((row) => Number(row.getAttribute("aria-rowindex")));
	expect(numbers).toEqual([1, 2, 3, 4, 5, 6]);
	await expect
		.element(page.getByRole("table"))
		.toHaveAttribute("aria-rowcount", "6");
});

// Kills an aside column left under the measure's header.
test("an aside column takes the measure's place under its own header", async () => {
	mount(3, () => {}, {
		label: "Report",
		column: "lr-c-measure",
		cell: (line) => <span>{line.benchmark.name}</span>,
	});
	await expect
		.element(page.getByRole("columnheader", { name: "Report" }))
		.toBeVisible();
	expect(
		page.getByRole("columnheader", { name: "Measure" }).query(),
	).toBeNull();
});
