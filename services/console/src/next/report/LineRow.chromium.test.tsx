import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "./report.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { HEIGHTS } from "./LineTable";
import LineRow from "./LineRow";
import { linesOf } from "./lines";
import { lineFixture, reportFixture } from "./testing";

const NAME = "blake3 input_bytes=65536 simd=avx2 threads=1 Latency";

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
		expect(height(1)).toBe(heights.plot);
	},
);

// Kills touch targets under 44 px on a phone.
test("a narrow row's controls are 44 px targets", async () => {
	mount({ narrow: true, onSelect: () => {} });
	for (const target of [
		page.getByRole("checkbox").element().parentElement as HTMLElement,
		page.getByRole("button", { name: `Expand ${NAME}` }).element(),
	]) {
		const { width, height } = target.getBoundingClientRect();
		expect(width).toBeGreaterThanOrEqual(44);
		expect(height).toBeGreaterThanOrEqual(44);
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
