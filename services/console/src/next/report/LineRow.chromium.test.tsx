import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "./report.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import { HEIGHTS } from "./LineTable";
import LineRow, { RowExplore } from "./LineRow";
import { linesOf } from "./lines";
import { lineFixture, reportFixture } from "./testing";

const NAME = "blake3 input_bytes=65536 simd=avx2 threads=1 Latency";
const UNCHECKED = "blake3 input_bytes=65536 simd=avx2 threads=1 Throughput";
const EXPLORE = "/console/projects/hashbrown/explore?benchmarks=b";

let dispose: (() => void) | undefined;

beforeEach(async () => {
	await page.viewport(1280, 800);
	document.documentElement.setAttribute(THEME_ATTRIBUTE, "dark");
});

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const [checked, unchecked] = linesOf(
	reportFixture([
		lineFixture({
			alert: { uuid: "a", limit: "upper", status: "active" } as never,
		}),
		lineFixture({
			measure: 1,
			model: undefined,
			baseline: undefined,
			upper_limit: undefined,
		}),
	]),
);

const mount = (
	props: Partial<Parameters<typeof LineRow>[0]> & { narrow?: boolean } = {},
) => {
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	const [expanded, setExpanded] = createSignal(false);
	const onExpand = vi.fn(setExpanded);
	dispose = render(
		() => (
			<table
				class="lr-table"
				classList={{ "lr-narrow": props.narrow === true }}
				aria-label="Lines"
			>
				{/* A narrow list's columns, as LineTable lays them out. */}
				{props.narrow === true && (
					<colgroup>
						<col class="lr-c-hit" />
						<col />
						<col class="lr-c-numbers" />
						<col class="lr-c-hit" />
					</colgroup>
				)}
				<tbody>
					<LineRow
						line={checked as never}
						metrics={false}
						narrow={false}
						index={2}
						expanded={expanded()}
						onExpand={onExpand}
						{...props}
					>
						<p>The plot</p>
					</LineRow>
				</tbody>
			</table>
		),
		root,
	);
	return { onExpand };
};

// Kills an alerting row told by its color alone, and a delta that drops its arrow or word.
test("an alerting row says so, and its delta names its direction", async () => {
	mount();
	const row = page.getByRole("row");
	await expect
		.element(row.getByRole("img", { name: "alerting" }))
		.toBeVisible();
	await expect
		.element(row.getByRole("cell").nth(5))
		.toHaveTextContent("20.60 ns");
	await expect
		.element(row.getByRole("cell").nth(6))
		.toHaveTextContent("↑ +6.2% worse");
	await expect
		.element(row.getByRole("cell").nth(7))
		.toHaveTextContent("19.90 ns");
});

// Kills a delta or a limit drawn for a line no threshold checks, and a link that opens nothing.
test("a line no threshold checks offers the snippet instead of a limit", async () => {
	const onNoThreshold = vi.fn();
	mount({ line: unchecked as never, onNoThreshold });
	const row = page.getByRole("row");
	expect(row.getByRole("cell").nth(6).element().textContent).toBe("");
	await row
		.getByRole("button", {
			name: "No threshold checks blake3 input_bytes=65536 simd=avx2 threads=1 Throughput. Show the run snippet that declares one.",
		})
		.click();
	expect(onNoThreshold).toHaveBeenCalledOnce();
	await expect
		.element(row.getByRole("img", { name: "alerting" }))
		.not.toBeInTheDocument();
});

// Kills a narrow row that drops the link to the snippet.
test("a narrow line no threshold checks offers the snippet instead of a limit", async () => {
	const onNoThreshold = vi.fn();
	mount({ line: unchecked as never, narrow: true, onNoThreshold });
	await page
		.getByRole("button", {
			name: "No threshold checks blake3 input_bytes=65536 simd=avx2 threads=1 Throughput. Show the run snippet that declares one.",
		})
		.click();
	expect(onNoThreshold).toHaveBeenCalledOnce();
	await expect.element(page.getByText(/^limit /)).not.toBeInTheDocument();
});

// Kills a line with no threshold to declare drawn with an empty limit, or with
// a link that opens nothing, wide or narrow.
test("a line no threshold checks and none can be declared for reads no threshold", async () => {
	for (const narrow of [false, true]) {
		mount({ line: unchecked as never, narrow });
		await expect
			.element(page.getByRole("row").getByText("no threshold", { exact: true }))
			.toBeVisible();
		await expect
			.element(page.getByRole("button", { name: /^No threshold checks / }))
			.not.toBeInTheDocument();
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a checkbox named for nothing, one that reports the old state, and one drawn without a handler.
test("the checkbox names the line and reports what it became", async () => {
	const onSelect = vi.fn();
	mount({ onSelect, selected: false });
	await page.getByRole("checkbox", { name: `Select ${NAME}` }).click();
	expect(onSelect).toHaveBeenCalledWith(true);

	dispose?.();
	document.body.replaceChildren();
	mount();
	await expect.element(page.getByRole("checkbox")).not.toBeInTheDocument();
});

// Kills an expand control that does not say its state, a region it does not
// control, and children drawn while the row is shut.
test("the expand control opens the row into the region it controls", async () => {
	const { onExpand } = mount();
	await expect.element(page.getByText("The plot")).not.toBeInTheDocument();
	const twist = page.getByRole("button", { name: `Expand ${NAME}` });
	await expect.element(twist).toHaveAttribute("aria-expanded", "false");
	await twist.click();
	expect(onExpand).toHaveBeenCalledWith(true);

	const region = page.getByRole("region", { name: `${NAME}, full plot` });
	await expect.element(region).toHaveTextContent("The plot");
	const collapse = page.getByRole("button", { name: `Collapse ${NAME}` });
	await expect.element(collapse).toHaveAttribute("aria-expanded", "true");
	await expect
		.element(collapse)
		.toHaveAttribute("aria-controls", region.element().id);
});

// Kills a row whose drawn height drifts from the one the list lays out by.
test.each([
	["wide", false],
	["narrow", true],
] as const)(
	"a %s row is drawn at the heights the list lays out by",
	async (width, narrow) => {
		const heights = HEIGHTS[width];
		mount({ narrow });
		const height = (index: number) =>
			page.getByRole("row").nth(index).element().getBoundingClientRect().height;
		expect(height(0)).toBe(heights.line);
		await page.getByRole("button", { name: `Expand ${NAME}` }).click();
		await expect.element(page.getByText("The plot")).toBeVisible();
		expect(height(0)).toBe(heights.line);
		const identity = document.querySelector<HTMLElement>(".lr-id");
		expect(identity?.offsetHeight).toBeGreaterThan(0);
		expect(height(1)).toBe(heights.plot + (identity?.offsetHeight ?? 0));
	},
);

// Kills touch targets under 44 px on a phone.
test("a narrow row's controls are 44 px targets", async () => {
	mount({ narrow: true, onSelect: () => {}, explore: EXPLORE });
	const explore = page.getByRole("link", { name: `Open in Explore: ${NAME}` });
	await expect.element(explore).toBeVisible();
	for (const target of [
		page.getByRole("checkbox").element().parentElement as HTMLElement,
		page.getByRole("button", { name: `Expand ${NAME}` }).element(),
		explore.element(),
	]) {
		const { width, height } = target.getBoundingClientRect();
		expect(width).toBeGreaterThanOrEqual(44);
		expect(height).toBeGreaterThanOrEqual(44);
	}
});

// Kills a narrow limit left on the measure's line, where a phone pushes it
// off the row, rather than under the value in the numbers' cell.
test("a narrow row's limit sits under its value, inside its cell", async () => {
	await page.viewport(320, 800);
	mount({ narrow: true });
	const value = page.getByText("20.60 ns").element() as HTMLElement;
	const limit = value.parentElement?.lastElementChild as HTMLElement;
	expect(limit.textContent).toBe("limit 19.90 ns");
	const cell = (value.closest("td") as HTMLElement).getBoundingClientRect();
	const box = limit.getBoundingClientRect();
	expect(box.top).toBeGreaterThan(value.getBoundingClientRect().bottom - 1);
	expect(box.right).toBeLessThanOrEqual(cell.right);
	expect(box.bottom).toBeLessThanOrEqual(cell.bottom);
});

// Kills a long measure that pushes the history off its line.
test("a narrow row's long measure gives way to its history", async () => {
	await page.viewport(390, 800);
	mount({
		narrow: true,
		line: {
			...checked,
			measure: { ...checked?.measure, name: "m".repeat(64) },
		} as never,
	});
	const line = document.querySelector<HTMLElement>(".lrn-l2") as HTMLElement;
	expect(line.scrollWidth).toBeLessThanOrEqual(line.clientWidth);
	const history = line.querySelector(".pl-inline") as Element;
	expect(history.getBoundingClientRect().right).toBeLessThanOrEqual(
		line.getBoundingClientRect().right,
	);
});

// Kills a number cell that overflows at its start, where the sign and the magnitude are.
test("a narrow row's numbers that overflow their cell keep their start", async () => {
	await page.viewport(320, 800);
	mount({ narrow: true });
	const value = page.getByText("20.60 ns").element() as HTMLElement;
	const delta = value.nextElementSibling as HTMLElement;
	value.textContent = "-123,456,789.12";
	delta.textContent = "↑ +999.9% better";
	const cell = (value.closest("td") as HTMLElement).getBoundingClientRect();
	// The value clips in its own box, so its cut is at the end however the cell aligns it.
	expect(value.clientWidth).toBeLessThan(value.scrollWidth);
	for (const number of [value, delta]) {
		const start = document.createRange();
		start.setStart(number.firstChild as Text, 0);
		start.setEnd(number.firstChild as Text, 1);
		const drawn = start.getBoundingClientRect();
		expect(number.scrollWidth, number.textContent).toBeGreaterThan(cell.width);
		expect(drawn.left, number.textContent).toBeGreaterThanOrEqual(cell.left);
		expect(drawn.left, number.textContent).toBeGreaterThanOrEqual(
			number.getBoundingClientRect().left,
		);
	}
});

// Kills an aside that only the wide row draws, or that leaves the measure beside it.
test("an aside takes the measure's place in both layouts", async () => {
	for (const narrow of [false, true]) {
		mount({ narrow, aside: <a href="/report">Sep 13</a> });
		const row = page.getByRole("row");
		await expect
			.element(row.getByRole("link", { name: "Sep 13" }))
			.toBeVisible();
		expect(row.element().textContent).not.toContain("Latency");
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a dimmed row faded by opacity, one still drawn as alerting, and a dim that changes nothing.
test("a dimmed row is drawn in the muted color, not faded, and does not alert", async () => {
	mount({ dimmed: true });
	const row = page.getByRole("row");
	await expect
		.element(row.getByRole("img", { name: "alerting" }))
		.not.toBeInTheDocument();
	const probe = document.createElement("span");
	probe.style.color = "var(--color-text-muted)";
	probe.style.backgroundColor = "var(--color-error-muted)";
	document.body.append(probe);
	const muted = getComputedStyle(probe).color;
	const alerting = getComputedStyle(probe).backgroundColor;
	const element = row.element();
	expect(
		getComputedStyle(element.querySelector("td") as HTMLElement)
			.backgroundColor,
	).not.toBe(alerting);
	const name = element.querySelector("b") as HTMLElement;
	expect(getComputedStyle(name).color).toBe(muted);
	expect(
		getComputedStyle(element.querySelector("td") as HTMLElement).color,
	).toBe(muted);
	for (const each of [element, ...element.querySelectorAll("td")]) {
		expect(getComputedStyle(each).opacity).toBe("1");
	}
});

// Kills controls left out of a row, a wide row that does not give them a
// cell of their own or an expanded plot that does not span it, and a narrow
// row that does not grow by the line the list lays out for them.
test.each([
	["wide", false],
	["narrow", true],
] as const)("a %s row's own controls", async (width, narrow) => {
	mount({ narrow, actions: () => <button type="button">Reactivate</button> });
	const row = page.getByRole("row").nth(0);
	await expect
		.element(row.getByRole("button", { name: "Reactivate" }))
		.toBeVisible();
	const heights = HEIGHTS[width];
	expect(row.element().getBoundingClientRect().height).toBe(
		heights.line + heights.actions,
	);
	if (!narrow) {
		await expect
			.element(row.getByRole("cell").nth(9).getByRole("button"))
			.toHaveTextContent("Reactivate");
	}
	await page.getByRole("button", { name: `Expand ${NAME}` }).click();
	const cell = page.getByRole("row").nth(1).getByRole("cell").element();
	expect((cell as HTMLTableCellElement).colSpan).toBe(narrow ? 4 : 10);
});

// Kills a row whose text or space does not toggle its plot, wide or narrow,
// and one that only ever opens it.
test.each([
	["wide", false],
	["narrow", true],
] as const)(
	"a click on a %s row's text toggles its plot",
	async (_, narrow) => {
		const { onExpand } = mount({ narrow });
		const measure = () =>
			page.getByRole("row").nth(0).getByText("Latency", { exact: true });
		await measure().click();
		expect(onExpand).toHaveBeenLastCalledWith(true);
		await expect.element(page.getByText("The plot")).toBeVisible();
		await measure().click();
		expect(onExpand).toHaveBeenLastCalledWith(false);
		await expect.element(page.getByText("The plot")).not.toBeInTheDocument();
		expect(onExpand).toHaveBeenCalledTimes(2);
	},
);

// Kills a row that toggles its plot on a click meant for its checkbox, its
// snippet button, its Explore link, or a tooltip drawn inside it, and a caret
// whose click the row toggles back.
test("a row's controls and the tooltip inside it keep their clicks", async () => {
	const onSelect = vi.fn();
	const onNoThreshold = vi.fn();
	const { onExpand } = mount({
		line: unchecked as never,
		onSelect,
		onNoThreshold,
		explore: EXPLORE,
	});
	const row = page.getByRole("row").nth(0);
	await row.getByRole("checkbox").click();
	expect(onSelect).toHaveBeenCalledWith(true);
	await row.getByRole("button", { name: /^No threshold checks / }).click();
	expect(onNoThreshold).toHaveBeenCalledOnce();
	const explore = row.getByRole("link", {
		name: `Open in Explore: ${UNCHECKED}`,
	});
	await expect.element(explore).toBeVisible();
	const stay = (event: Event) => event.preventDefault();
	document.addEventListener("click", stay, { capture: true });
	await explore.click();
	(row.element().querySelector("[role=tooltip]") as HTMLElement).click();
	document.removeEventListener("click", stay, { capture: true });
	expect(onExpand).not.toHaveBeenCalled();
	await row.getByRole("button", { name: `Expand ${UNCHECKED}` }).click();
	expect(onExpand).toHaveBeenCalledOnce();
	expect(onExpand).toHaveBeenCalledWith(true);
});

// Kills a row that toggles its plot as a drag selecting its text ends.
test("a click that ends selecting the row's text leaves the plot alone", async () => {
	const { onExpand } = mount();
	const measure = page
		.getByRole("row")
		.nth(0)
		.getByText("Latency", { exact: true })
		.element() as HTMLElement;
	getSelection()?.selectAllChildren(measure);
	measure.click();
	expect(onExpand).not.toHaveBeenCalled();
	getSelection()?.removeAllRanges();
	measure.click();
	expect(onExpand).toHaveBeenCalledWith(true);
});

// Kills a row without its own link into Explore, one drawn before the page can
// write it, and one that opens anything but the line's link.
test("a row opens its line in Explore once the page can write the link", async () => {
	const [href, setHref] = createSignal<string>();
	mount({
		get explore() {
			return href();
		},
	});
	const row = page.getByRole("row").nth(0);
	await RowExplore.preload();
	await new Promise((resolve) => requestAnimationFrame(resolve));
	expect(row.getByRole("link").elements()).toHaveLength(0);
	setHref(EXPLORE);
	await expect
		.element(row.getByRole("link", { name: `Open in Explore: ${NAME}` }))
		.toHaveAttribute("href", EXPLORE);
});

// Kills a row that names an alert without linking it, a link to another
// place, a click on the link the row takes for its toggle, and a row whose
// other cells no longer toggle its plot.
test.each([
	["wide", false],
	["narrow", true],
] as const)(
	"a %s row naming an alert links its name to the alert, and the rest of the row toggles its plot",
	async (_, narrow) => {
		const { onExpand } = mount({ narrow, href: "/alerts/a" });
		const row = page.getByRole("row").nth(0);
		const link = row.getByRole("link", { name: `${NAME}, open the alert` });
		await expect.element(link).toHaveAttribute("href", "/alerts/a");
		const stay = (event: Event) => event.preventDefault();
		document.addEventListener("click", stay, { capture: true });
		await link.click();
		document.removeEventListener("click", stay, { capture: true });
		expect(onExpand).not.toHaveBeenCalled();
		await row.getByText("Latency", { exact: true }).click();
		expect(onExpand).toHaveBeenCalledWith(true);
	},
);

// Kills a row that takes a tab stop of its own, and controls out of the order
// they are drawn in, wide or narrow.
test.each([
	["wide", false, false, ["Select", "Expand", "Open"]],
	["narrow", true, false, ["Select", "Open", "Expand"]],
	["wide alert's", false, true, ["Select", "Expand", "blake3", "Open"]],
	["narrow alert's", true, true, ["Select", "blake3", "Open", "Expand"]],
] as const)(
	"a %s row's tab stops are its controls in order",
	async (_, narrow, alert, order) => {
		mount({
			narrow,
			onSelect: () => {},
			explore: EXPLORE,
			...(alert ? { href: "/alerts/a" } : {}),
		});
		await expect
			.element(page.getByRole("link", { name: `Open in Explore: ${NAME}` }))
			.toBeVisible();
		const stops: string[] = [];
		(document.activeElement as HTMLElement | null)?.blur();
		for (const _ of order) {
			await userEvent.keyboard("{Tab}");
			const focused = document.activeElement as HTMLElement;
			stops.push(
				(focused.getAttribute("aria-label") ?? focused.textContent ?? "").split(
					" ",
				)[0] ?? "",
			);
		}
		expect(stops).toEqual(order);
	},
);
