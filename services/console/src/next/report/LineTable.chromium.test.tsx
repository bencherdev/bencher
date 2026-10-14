import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "./report.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
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
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

const mount = (
	count: number,
	onNearEnd = () => {},
	extra: Partial<Parameters<typeof LineTable>[0]> = {},
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
				{...extra}
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
	const reserved = space.getBoundingClientRect().height;
	expect(space.querySelector(".pl")).toBeNull();
	await expect
		.element(
			page
				.getByRole("region", { name: "bench-0 n=0 Latency, full plot" })
				.getByRole("button", { name: /: hide its line$/ }),
		)
		.toBeVisible();
	expect(space.getBoundingClientRect().height).toBe(reserved);
});

// Kills a reserved height short of the plot and its key, which cuts off the
// bottom of the key's entries, or one that leaves a gap under them.
for (const [width, narrow] of [
	[1280, false],
	[390, true],
] as const) {
	test(`an expanded row's plot fills its space, key and all, at ${width}`, async () => {
		await page.viewport(width, 800);
		mount(3, () => {}, { narrow });
		await page
			.getByRole("button", { name: "Expand bench-0 n=0 Latency" })
			.click();
		const region = page.getByRole("region", {
			name: "bench-0 n=0 Latency, full plot",
		});
		const toggle = region.getByRole("button", { name: /: hide its line$/ });
		const target = region.getByRole("button", { name: /^Focus .* boundary$/ });
		await expect.element(toggle).toBeVisible();
		const space = region.element().getBoundingClientRect();
		for (const control of [toggle, target]) {
			expect(control.element().getBoundingClientRect().bottom).toBeLessThan(
				space.bottom,
			);
		}
		const plot = region.getByRole("figure").element().getBoundingClientRect();
		expect(plot.height).toBe(space.height);
	});
}

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
			.getByRole("button", { name: /: hide its line$/ });
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
		aside: {
			label: "Report",
			column: "lr-c-measure",
			cell: (line) => <span>{line.benchmark.name}</span>,
		},
	});
	await expect
		.element(page.getByRole("columnheader", { name: "Report" }))
		.toBeVisible();
	expect(
		page.getByRole("columnheader", { name: "Measure" }).query(),
	).toBeNull();
});

// Kills a page's group header left unused, or laid out at the report's
// header height so the rows below it drift from where the list puts them.
test("a page's own group header replaces the report's, at its own height", async () => {
	mount(200, undefined, {
		group: {
			height: { wide: 50, narrow: 70 },
			row: (props) => (
				<tr aria-rowindex={props.index}>
					<td colSpan={props.columns} style={{ height: "50px", padding: "0" }}>
						Report {props.group.name}
					</td>
				</tr>
			),
		},
	});
	await expect.element(page.getByText("Report bench-0")).toBeVisible();
	await expect
		.element(page.getByText(/10 variants/).first())
		.not.toBeInTheDocument();
	await frame();
	const body = document.querySelector("tbody") as HTMLElement;
	expect(body.getBoundingClientRect().height).toBe(20 * 50 + 200 * 44);
});

// Kills row controls without a column, a header, or their own row's cell, a
// group header that stops short of their column, and a dim the list does not
// pass to its row.
test("a list with row controls gives them a column, and dims the rows it names", async () => {
	const { lines } = mount(3, undefined, {
		actions: {
			cell: (line) => <button type="button">Act on {line.variant}</button>,
			narrow: () => false,
		},
		dimmed: (line) => line.variant === "v1",
	});
	await expect
		.element(page.getByRole("columnheader", { name: "Actions" }))
		.toBeInTheDocument();
	// A group's header spans the controls' column too.
	const header = page.getByRole("row").nth(1).getByRole("cell").element();
	expect((header as HTMLTableCellElement).colSpan).toBe(10);
	const row = (variant: string) =>
		page
			.getByRole("row")
			.filter({ has: page.getByRole("button", { name: `Act on ${variant}` }) });
	await expect
		.element(row("v0").getByRole("cell").nth(9))
		.toHaveTextContent("Act on v0");
	const color = (variant: string) =>
		getComputedStyle(row(variant).element().querySelector("b") as HTMLElement)
			.color;
	expect(color("v1")).not.toBe(color("v0"));
	expect(lines).toHaveLength(3);
});

// Kills a narrow list that gives every row a line for its controls, or none.
test("a narrow list draws a line of controls only for the rows it names", async () => {
	await page.viewport(390, 800);
	mount(3, undefined, {
		narrow: true,
		actions: {
			cell: (line) => <button type="button">Act on {line.variant}</button>,
			narrow: (line) => line.variant === "v1",
		},
	});
	await expect
		.element(page.getByRole("button", { name: "Act on v1" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "Act on v0" }))
		.not.toBeInTheDocument();
	expect(document.querySelectorAll("tr.lrn-act")).toHaveLength(1);
});

// Kills row links drawn, or the page's codec asked for, before the page is
// idle or the reader moves toward the lines; a pointer the page moved under
// taken for the reader's; a list that hands a row's link every line, another
// row's line, or none; and one that draws no column for the links.
test.each([
	[
		"a pointer moving onto them",
		async () => {
			await userEvent.hover(page.getByRole("columnheader", { name: "Line" }));
			await userEvent.hover(
				page.getByRole("button", { name: "Expand bench-0 n=1 Latency" }),
			);
		},
	],
	["the keyboard entering them", () => userEvent.keyboard("{Tab}")],
] as const)(
	"each row's Explore link, once %s, is the page's link for that one line",
	async (_, move) => {
		// A page that never goes idle, so only the reader can bring the links.
		vi.stubGlobal("requestIdleCallback", () => 0);
		vi.stubGlobal("cancelIdleCallback", () => {});
		const onExploreIntent = vi.fn();
		const { lines } = mount(3, undefined, {
			explore: (chosen) => `/explore?lines=${chosen.map(({ key }) => key)}`,
			onExploreIntent,
		});
		await expect
			.element(page.getByRole("columnheader", { name: "Explore" }))
			.toBeInTheDocument();
		page
			.getByRole("table")
			.element()
			.dispatchEvent(new PointerEvent("pointermove", { bubbles: true }));
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(page.getByRole("link").elements()).toHaveLength(0);
		expect(onExploreIntent).not.toHaveBeenCalled();
		await move();
		expect(onExploreIntent).toHaveBeenCalledOnce();
		for (const [index, line] of lines.entries()) {
			await expect
				.element(
					page.getByRole("link", {
						name: `Open in Explore: bench-0 n=${index} Latency`,
					}),
				)
				.toHaveAttribute("href", `/explore?lines=${line.key}`);
		}
	},
);

// Kills row links that never arrive for a reader who neither points nor
// focuses, such as a screen reader reading the page.
test("each row's Explore link arrives once the page goes idle, with no pointer or focus", async () => {
	const onExploreIntent = vi.fn();
	const { lines } = mount(3, undefined, {
		explore: (chosen) => `/explore?lines=${chosen.map(({ key }) => key)}`,
		onExploreIntent,
	});
	for (const [index, line] of lines.entries()) {
		await expect
			.element(
				page.getByRole("link", {
					name: `Open in Explore: bench-0 n=${index} Latency`,
				}),
			)
			.toHaveAttribute("href", `/explore?lines=${line.key}`);
	}
	expect(onExploreIntent).toHaveBeenCalledOnce();
});

/** Holds the page's idle callback until the test runs it, and drops it when the page cancels it. */
const holdIdle = () => {
	let held: (() => void) | undefined;
	vi.stubGlobal("requestIdleCallback", (then: () => void) => {
		held = then;
		return 1;
	});
	vi.stubGlobal("cancelIdleCallback", () => {
		held = undefined;
	});
	return () => held?.();
};

/** The idle load marks the page has set. */
const idleMarks = (mark: { mock: { calls: unknown[][] } }) =>
	mark.mock.calls.filter(([name]) => name === "console:idle-load");

const explore = (chosen: readonly ReportLine[]) =>
	`/explore?lines=${chosen.map(({ key }) => key)}`;

// Kills an idle load that outlives its table, which would load the links' code
// and set its mark on whatever page the reader moved to.
test("a table gone before the page goes idle loads nothing and marks nothing", async () => {
	const idle = holdIdle();
	const mark = vi.spyOn(performance, "mark");
	const stays = vi.fn();
	mount(3, undefined, { explore, onExploreIntent: stays });
	await expect.element(page.getByRole("table")).toBeInTheDocument();
	idle();
	expect(idleMarks(mark)).toHaveLength(1);
	expect(stays).toHaveBeenCalledOnce();
	dispose?.();
	document.body.replaceChildren();

	const leaves = vi.fn();
	mount(3, undefined, { explore, onExploreIntent: leaves });
	await expect.element(page.getByRole("table")).toBeInTheDocument();
	dispose?.();
	dispose = undefined;
	idle();
	expect(idleMarks(mark)).toHaveLength(1);
	expect(leaves).not.toHaveBeenCalled();
});

// Kills an idle load that marks and asks again after the reader already
// brought the links, so the mark no longer says where the idle load began.
test("once the reader brought the links, the page going idle marks and asks nothing", async () => {
	const idle = holdIdle();
	const mark = vi.spyOn(performance, "mark");
	const onExploreIntent = vi.fn();
	mount(3, undefined, { explore, onExploreIntent });
	await expect.element(page.getByRole("table")).toBeInTheDocument();
	await userEvent.keyboard("{Tab}");
	expect(onExploreIntent).toHaveBeenCalledOnce();
	idle();
	expect(idleMarks(mark)).toHaveLength(0);
	expect(onExploreIntent).toHaveBeenCalledOnce();
});

// Kills a page that never goes idle where the browser has no idle callback,
// as in Safari, so a reader who only waits there never gets the links.
test("where the browser has no idle callback, the links still arrive", async () => {
	let owner: object | null = window;
	while (owner && !Object.hasOwn(owner, "requestIdleCallback")) {
		owner = Object.getPrototypeOf(owner);
	}
	const own =
		owner && Object.getOwnPropertyDescriptor(owner, "requestIdleCallback");
	if (owner) {
		delete (owner as { requestIdleCallback?: unknown }).requestIdleCallback;
	}
	try {
		expect("requestIdleCallback" in window).toBe(false);
		mount(3, undefined, { explore });
		await expect
			.element(
				page.getByRole("link", {
					name: "Open in Explore: bench-0 n=0 Latency",
				}),
				{ timeout: 3_000 },
			)
			.toBeVisible();
	} finally {
		if (owner && own) {
			Object.defineProperty(owner, "requestIdleCallback", own);
		}
	}
});
