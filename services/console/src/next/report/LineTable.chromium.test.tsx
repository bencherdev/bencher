import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "./report.css";
import "../thresholds/thresholds.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import { FOLD, STACK } from "./fold";
import LineTable from "./LineTable";
import { type ReportLine, groupsOf, linesOf, plotOf } from "./lines";
import { slotsOf } from "./slots";
import { HEIGHTS } from "./LineTable";
import { lineFixture, longReport, reportFixture } from "./testing";

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
	report = longReport(count),
) => {
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
				fold={880}
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
		.element(page.getByRole("button", { name: "Expand bench-49 n=9" }))
		.not.toBeInTheDocument();

	bottom();
	await expect
		.element(page.getByRole("button", { name: "Expand bench-49 n=9" }))
		.toBeInTheDocument();
	expect(drawnRows()).toBeLessThan(45);
	await expect
		.element(page.getByRole("button", { name: "Expand bench-0 n=0" }))
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
		name: "Expand bench-0 n=0",
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
				.getByRole("region", { name: "bench-0 n=0, full plot" })
				.getByRole("button", { name: /: hide its line$/ }),
		)
		.toBeVisible();
	expect(space.getBoundingClientRect().height).toBe(reserved);
});

// Kills a reserved height short of the plot and its key, which cuts off the
// bottom of the key's entries, or one that leaves a gap under them.
for (const width of [1280, 390]) {
	test(`an expanded row's plot fills its space, key and all, at ${width}`, async () => {
		await page.viewport(width, 800);
		mount(3);
		await page.getByRole("button", { name: "Expand bench-0 n=0" }).click();
		const region = page.getByRole("region", {
			name: "bench-0 n=0, full plot",
		});
		const toggle = region.getByRole("button", { name: /: hide its line$/ });
		await expect.element(toggle).toBeVisible();
		const space = region.element().getBoundingClientRect();
		expect(toggle.element().getBoundingClientRect().bottom).toBeLessThan(
			space.bottom,
		);
		const plot = region.getByRole("figure").element().getBoundingClientRect();
		expect(plot.height).toBe(space.height);
	});
}

// Kills offsets that leave out an open row's plot, or count it at another
// height than it draws, so the list moves once the row scrolls away.
for (const width of [1280, 390]) {
	test(`an open row keeps its height once it scrolls out of view at ${width}`, async () => {
		await page.viewport(width, 800);
		mount(200);
		await page.getByRole("button", { name: "Expand bench-0 n=0" }).click();
		await expect
			.element(page.getByRole("region", { name: "bench-0 n=0, full plot" }))
			.toBeInTheDocument();
		const height = () =>
			page.getByRole("table").element().getBoundingClientRect().height;
		const open = height();
		bottom();
		await expect
			.element(page.getByRole("button", { name: "Expand bench-19 n=9" }))
			.toBeInTheDocument();
		expect(height()).toBe(open);
	});
}

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
			.getByRole("button", { name: /^Expand bench-0 n=0$/ })
			.element() as HTMLElement;
		const start = performance.now();
		twist.click();
		document.body.getBoundingClientRect();
		expands.push(performance.now() - start);

		const hide = page
			.getByRole("region", { name: "bench-0 n=0, full plot" })
			.getByRole("button", { name: /: hide its line$/ });
		await expect.element(hide).toBeVisible();
		await frame();
		const toggle = performance.now();
		(hide.element() as HTMLElement).click();
		document.body.getBoundingClientRect();
		toggles.push(performance.now() - toggle);

		(
			page
				.getByRole("button", { name: "Collapse bench-0 n=0" })
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
	await page.getByRole("button", { name: "Expand bench-0 n=1" }).click();
	await expect
		.element(page.getByRole("region", { name: "bench-0 n=1, full plot" }))
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
				page.getByRole("button", { name: "Expand bench-0 n=1" }),
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
						name: `Open in Explore: bench-0 n=${index}`,
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
					name: `Open in Explore: bench-0 n=${index}`,
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
					name: "Open in Explore: bench-0 n=0",
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

/** 16 characters of the row's name, in the engine where they are widest. */
const FLOOR = 131.2;

/** The width of the Line column's text, or of a folded row's name, with the table `width` wide. */
const nameAt = async (box: HTMLElement, width: number, layout: string) => {
	box.style.width = `${width}px`;
	const table = page.getByRole("table").element();
	await expect
		.poll(() =>
			table.classList.contains("lr-stacked")
				? "stacked"
				: table.classList.contains("lr-narrow")
					? "narrow"
					: "wide",
		)
		.toBe(layout);
	const line = [...table.querySelectorAll("th")].find(
		(th) => th.textContent === "Line",
	);
	if (line) {
		const style = getComputedStyle(line);
		return (
			line.getBoundingClientRect().width -
			Number.parseFloat(style.paddingLeft) -
			Number.parseFloat(style.paddingRight)
		);
	}
	return (
		table.querySelector(".lrn-l1")?.getBoundingClientRect().width ?? Number.NaN
	);
};

// Kills a fold or a stack keyed to the screen rather than the table, and a
// width that lets the name fall under 16 characters or folds a pixel early.
test.each([
	["report", FOLD.report, { selected: new Set<string>(), onSelect: () => {} }],
	[
		"alerts",
		FOLD.alerts,
		{
			selected: new Set<string>(),
			onSelect: () => {},
			actions: {
				cell: () => <span>Dismiss</span>,
				narrow: (): boolean => false,
			},
		},
	],
	[
		"run",
		FOLD.run,
		{ beside: { label: "Report", column: "th-c-report", cell: () => "#1" } },
	],
	[
		"threshold",
		FOLD.threshold,
		{
			aside: { label: "Report", column: "th-c-report", cell: () => "Sep 1" },
			actions: {
				cell: () => <span>Dismiss</span>,
				narrow: (): boolean => false,
			},
		},
	],
] as const)(
	"the %s list folds and stacks by its own width, its name at least 16 characters",
	async (list, fold, extra) => {
		// Under 1280 px, where the Report column is at its narrower width.
		await page.viewport(1200, 800);
		const box = document.createElement("div");
		box.className = list === "report" ? "" : "lr-alerts";
		document.body.append(box);
		dispose = render(
			() => (
				<LineTable
					label="Lines"
					slots={linesOf(longReport(3))}
					rows={3}
					fold={fold}
					metrics={false}
					history="History, 4w"
					expanded={new Set()}
					onExpand={() => {}}
					plot={(line) => ({
						data: plotOf(line, "main", "ubuntu-latest"),
						note: "4w",
						reportHref: (uuid) => `/reports/${uuid}`,
					})}
					{...(extra as Partial<Parameters<typeof LineTable>[0]>)}
				/>
			),
			box,
		);
		const stack = STACK["selected" in extra ? "select" : "plain"];
		for (const [width, layout] of [
			[fold + 1, "wide"],
			[stack + 1, "narrow"],
		] as const) {
			const name = await nameAt(box, width, layout);
			expect(name, `${layout} at ${width}`).toBeGreaterThanOrEqual(FLOOR);
			expect(name, `${layout} at ${width}`).toBeLessThan(FLOOR + 1);
		}
		await nameAt(box, fold, "narrow");
		expect(await nameAt(box, stack, "stacked")).toBeGreaterThanOrEqual(FLOOR);
		const row = document.querySelector("tr.lrn:not(.lrn-act)");
		expect(row?.getBoundingClientRect().height).toBe(HEIGHTS.stacked.line);
		// A 320 px phone's table: the value, delta, and limit each whole on their line.
		await nameAt(box, 292, "stacked");
		for (const number of document.querySelectorAll<HTMLElement>(
			".lrn-l3 > *",
		)) {
			expect(number.scrollWidth, number.textContent ?? "").toBeLessThanOrEqual(
				number.clientWidth,
			);
		}
		// Numbers at their longest: the limit gives way, inside its line.
		const numbers = document.querySelector<HTMLElement>(
			".lrn-l3",
		) as HTMLElement;
		const [value, delta, limit] = numbers.children as unknown as HTMLElement[];
		(value as HTMLElement).textContent = "-123,456,789";
		(delta as HTMLElement).textContent = "↑ +999.9%";
		expect((value as HTMLElement).scrollWidth).toBeLessThanOrEqual(
			(value as HTMLElement).clientWidth,
		);
		expect(limit?.getBoundingClientRect().right).toBeLessThanOrEqual(
			numbers.getBoundingClientRect().right,
		);
	},
);

const PATH = `my_crate::${"module_000::".repeat(80)}bench_fn_name_000`;

/** A report of 4 lines, each its own long benchmark with 8 long parameters, every line's tag `n` telling them apart. */
const taggedReport = () => {
	const report = reportFixture(
		Array.from({ length: 4 }, (_, index) =>
			lineFixture({
				variant: index,
				benchmark: index,
				alert: { uuid: `a${index}`, limit: "upper", status: "active" } as never,
			}),
		),
	);
	return {
		...report,
		benchmarks: Array.from({ length: 4 }, (_, index) => ({
			uuid: `b${index}`,
			name: `${PATH}${index + 1}`,
			slug: `b${index}`,
		})),
		variants: Array.from({ length: 4 }, (_, index) => ({
			uuid: `v${index}`,
			benchmark: index,
			parameters: Object.fromEntries(
				Array.from({ length: 8 }, (_, tag) => [
					`${tag}${"k".repeat(63)}`,
					`${index}${"v".repeat(63)}`,
				]),
			),
		})),
		groups: Array.from({ length: 4 }, (_, index) => ({
			key: index,
			lines: 99_999,
			variants: 99_999,
			alerts: 999_999,
		})),
	};
};

const mountTagged = (width: number, href?: string) => {
	const report = taggedReport();
	const [expanded, setExpanded] = createSignal<ReadonlySet<string>>(new Set());
	const box = document.createElement("div");
	box.className = "console";
	box.style.width = `${width}px`;
	document.body.append(box);
	dispose = render(
		() => (
			<LineTable
				label="Lines"
				slots={slotsOf(groupsOf(report, "benchmark"), linesOf(report))}
				rows={8}
				fold={FOLD.report}
				metrics={false}
				history="History, 4w"
				expanded={expanded()}
				onExpand={(line: ReportLine, open: boolean) =>
					setExpanded(new Set(open ? [line.key] : []))
				}
				selected={new Set()}
				onSelect={() => {}}
				{...(href === undefined ? {} : { href: () => href })}
				plot={(line) => ({
					data: plotOf(line, "main", "ubuntu-latest"),
					note: "4w",
					reportHref: (uuid) => `/reports/${uuid}`,
				})}
			/>
		),
		box,
	);
};

// Kills a phone row whose name, where its line cannot hold the tail beside
// the alert dot, pushes past the line rather than ending in an ellipsis, or
// has no way to its whole, which its expansion opens with.
test("at 320 px a name beside 8 long tags ends in an ellipsis, whole in its expansion", async () => {
	await page.viewport(320, 800);
	mountTagged(292);
	const cut = ".lrn-l1 b .ui-trunc[data-tail]";
	await expect.poll(() => document.querySelector(cut)).not.toBeNull();
	const name = document.querySelector<HTMLElement>(cut);
	expect(name?.getBoundingClientRect().width).toBeLessThan(
		textWidth(name as HTMLElement, name?.dataset.tail ?? ""),
	);
	name?.closest("tr")?.querySelector<HTMLElement>(".lr-twist")?.click();
	await expect
		.element(page.getByRole("region", { name: /full plot$/ }))
		.toBeInTheDocument();
	expect(document.querySelector(".lr-exrow .lr-id b")?.textContent).toBe(
		`${PATH}1`,
	);
});

// Kills a name held at its tail on a line too short for it, which pushes it,
// or the alert link holding it, past the line's end.
test("on a line shorter than a name's tail the name and its link stay inside it", async () => {
	await page.viewport(320, 800);
	for (const href of [undefined, "/alerts/a"]) {
		mountTagged(260, href);
		const cut = ".lrn-l1 b .ui-trunc[data-tail]";
		await expect.poll(() => document.querySelector(cut)).not.toBeNull();
		for (const name of document.querySelectorAll<HTMLElement>(cut)) {
			const line = name.closest(".lrn-l1")?.getBoundingClientRect();
			expect(name.getBoundingClientRect().right).toBeLessThanOrEqual(
				line?.right ?? 0,
			);
			expect(
				name.closest("a")?.getBoundingClientRect().right ?? 0,
			).toBeLessThanOrEqual(line?.right ?? 0);
		}
		dispose?.();
		dispose = undefined;
		document.body.replaceChildren();
	}
});

/** The width `text` takes in `element`'s font. */
const textWidth = (element: Element, text: string) => {
	const probe = document.createElement("span");
	probe.style.cssText = `position: absolute; white-space: nowrap; font: ${getComputedStyle(element).font}`;
	probe.textContent = text;
	document.body.append(probe);
	const width = probe.getBoundingClientRect().width;
	probe.remove();
	return width;
};

// Kills tags that each take a track of their own, or a name that gives way
// before its tags, which cuts an alerting row's tail until every row reads the
// same; a pill that cuts its value before its key; and, where the line has
// room, a first tag that shows no text.
for (const [width, table] of [
	[1440, 1392],
	[1024, 976],
	[390, 362],
] as const) {
	test(`at ${width} px each name's tail is whole beside 8 long tags`, async () => {
		await page.viewport(width, 800);
		mountTagged(table);
		await expect
			.poll(
				() =>
					document.querySelectorAll(
						".lr-line b .ui-trunc[data-tail], .lrn-l1 b .ui-trunc[data-tail]",
					).length,
			)
			.toBe(4);
		expect(document.querySelectorAll("svg.lr-alertdot")).toHaveLength(4);
		// The tail's copy that holds each name is never drawn.
		for (const hold of document.querySelectorAll(".lr-hold")) {
			expect(hold.checkVisibility({ visibilityProperty: true })).toBe(false);
		}
		for (const name of document.querySelectorAll<HTMLElement>(
			".lr-line b .ui-trunc[data-tail], .lrn-l1 b .ui-trunc[data-tail]",
		)) {
			const tail = name.dataset.tail ?? "";
			expect(tail).toMatch(/^bench_fn_name_000[1-4]$/);
			expect(name.getBoundingClientRect().width).toBeGreaterThan(
				textWidth(name, tail),
			);
		}
		for (const tag of document.querySelectorAll<HTMLElement>(
			".lr-tags .lr-ptag",
		)) {
			const [key, value] = tag.querySelectorAll<HTMLElement>(".ui-trunc");
			expect(value?.getBoundingClientRect().width).toBeGreaterThanOrEqual(
				Math.min(
					key?.getBoundingClientRect().width ?? 0,
					textWidth(tag, value?.textContent ?? ""),
				) - 0.5,
			);
		}
		if (width === 1440) {
			for (const tags of document.querySelectorAll<HTMLElement>(".lr-tags")) {
				const [key, value] = tags.querySelectorAll<HTMLElement>(
					".lr-ptag:first-child .ui-trunc",
				);
				const track = tags.getBoundingClientRect();
				expect(
					tags.querySelector(".lr-ptag")?.getBoundingClientRect().right,
				).toBeLessThanOrEqual(track.right);
				for (const part of [key, value]) {
					const box = part?.getBoundingClientRect();
					expect(box?.left).toBeLessThan(track.right);
					expect(box?.width).toBeGreaterThan(textWidth(tags, "kkkk"));
				}
			}
		}
	});
}

// Kills a header that cuts its alert count, or gives its counts no share
// beside a long name.
test("at 320 px a group header keeps its alert count whole beside a long name", async () => {
	await page.viewport(320, 800);
	mountTagged(292);
	await expect
		.poll(() => document.querySelector<HTMLElement>("tr.lr-group"))
		.not.toBeNull();
	const group = document.querySelector<HTMLElement>(
		"tr.lr-group",
	) as HTMLElement;
	const alerts = group.querySelector<HTMLElement>(".lr-galert") as HTMLElement;
	expect(alerts.scrollWidth).toBeLessThanOrEqual(alerts.clientWidth);
	expect(alerts.getBoundingClientRect().right).toBeLessThanOrEqual(
		group.getBoundingClientRect().right,
	);
	const [name, counts] = group.querySelectorAll<HTMLElement>(".ui-trunc");
	// The counts give way inside their own box, never over the alert count.
	const box = alerts.previousElementSibling?.previousElementSibling;
	expect(box?.scrollWidth).toBeLessThanOrEqual(box?.clientWidth ?? 0);
	expect(
		counts?.dataset.cut === undefined ||
			(counts?.getBoundingClientRect().width ?? 0) >=
				(name?.getBoundingClientRect().width ?? 0) - 0.5,
	).toBe(true);
});

// Kills an open row whose whole name is measured only while drawn, so a width
// change while it is off screen moves the list when it returns, and a measured
// copy that is drawn.
test("an open row resized while out of view keeps its place on its return", async () => {
	await page.viewport(1280, 800);
	const report = longReport(200);
	// A name that wraps to more lines on a phone than on a desktop.
	(report.benchmarks[0] as { name: string }).name =
		`bench-0 ${"segment ".repeat(60)}`;
	mount(200, undefined, {}, report);
	await page
		.getByRole("button", { name: /^Expand .*n=0$/ })
		.first()
		.click();
	await expect
		.element(page.getByRole("region", { name: /n=0, full plot$/ }))
		.toBeInTheDocument();
	// The copy measured for the list is never drawn.
	const measured = document.querySelector(".lr-measure .lr-id");
	expect(measured).not.toBeNull();
	expect(measured?.checkVisibility({ visibilityProperty: true })).toBe(false);
	bottom();
	await expect
		.element(page.getByRole("button", { name: "Expand bench-19 n=9" }))
		.toBeInTheDocument();
	await page.viewport(390, 800);
	const table = () => page.getByRole("table").element();
	await expect.poll(() => table().classList.contains("lr-narrow")).toBe(true);
	await new Promise((resolve) => requestAnimationFrame(resolve));
	const away = table().getBoundingClientRect().height;
	window.scrollTo({ top: 0, behavior: "instant" });
	await expect
		.element(page.getByRole("region", { name: /n=0, full plot$/ }))
		.toBeInTheDocument();
	for (const _ of [0, 1, 2]) {
		await new Promise((resolve) => requestAnimationFrame(resolve));
	}
	expect(table().getBoundingClientRect().height).toBe(away);
});
