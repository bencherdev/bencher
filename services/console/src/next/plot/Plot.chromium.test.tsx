import "@bencherdev/ui/styles.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import Plot, { type PlotProps } from "./Plot";
import { middle, near, pixelAt, rowsOf, settle, tokenRgb } from "./pixels";
import { returnSyncKey, takeSyncKey } from "./sync";
import { DAY, START, testData, testLine } from "./testing";
import type { PlotData } from "./types";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
	document.documentElement.removeAttribute(THEME_ATTRIBUTE);
});

beforeEach(async () => {
	await page.viewport(1280, 900);
	document.documentElement.setAttribute(THEME_ATTRIBUTE, "dark");
	// Park the pointer where no plot will mount, so a new key entry under it is not hovered.
	const park = document.createElement("div");
	park.style.cssText =
		"position: fixed; right: 0; bottom: 0; width: 4px; height: 4px";
	document.body.append(park);
	await page.elementLocator(park).hover();
	park.remove();
});

type Harness = Omit<
	PlotProps,
	"data" | "hidden" | "focused" | "onHiddenChange" | "onFocusChange"
> & {
	data: PlotData | undefined;
	hidden?: string[];
	focused?: string | null;
};

const mount = (props: Harness, width = 900) => {
	const root = document.createElement("div");
	root.style.width = `${width}px`;
	document.body.append(root);
	const [data, setData] = createSignal(props.data);
	const [hidden, setHidden] = createSignal<string[]>(props.hidden ?? []);
	const [focused, setFocused] = createSignal<string | null | undefined>(
		props.focused,
	);
	dispose = render(
		() => (
			<Plot
				{...props}
				data={data()}
				hidden={hidden()}
				focused={focused()}
				onHiddenChange={setHidden}
				onFocusChange={setFocused}
			/>
		),
		root,
	);
	return { root, setData, hidden, focused };
};

/** The plot area named `name`, its canvas, and its box on the page. */
const plotArea = async (name: RegExp | string = /lines?,/) => {
	const area = page.getByRole("group", { name });
	await expect.element(area).toBeInTheDocument();
	const over = area.element() as HTMLElement;
	const canvas = over.parentElement?.querySelector("canvas");
	if (!canvas) {
		throw new Error("The plot area has no canvas");
	}
	return { over, canvas, box: over.getBoundingClientRect() };
};

const series = (slot: number) => tokenRgb(`--color-data-categorical-${slot}`);

// Three flat lines at 10, 20, and 30 ns, one benchmark each so each takes its own slot.
const FLAT = testData([
	testLine({ id: "low", benchmark: "crc32c", y: [10, 10, 10, 10, 10] }),
	testLine({ id: "mid", benchmark: "sha256", y: [20, 20, 20, 20, 20] }),
	testLine({ id: "high", benchmark: "xxh3", y: [30, 30, 30, 30, 30] }),
]);

describe("Plot", () => {
	// Kills a wrong slot color, a missing line, or an inverted or nonlinear y scale.
	test("draws each line in its slot color where its values are", async () => {
		mount({ data: FLAT });
		await settle();
		const { canvas, box } = await plotArea("3 lines, Latency");
		const column = box.left + box.width / 2;
		const [low, mid, high] = [1, 2, 3].map((slot) =>
			middle(rowsOf(canvas, column, box.top, box.bottom, series(slot))),
		);
		expect(low).toBeGreaterThan(mid ?? 0);
		expect(mid).toBeGreaterThan(high ?? 0);
		expect((low ?? 0) - (mid ?? 0)).toBeCloseTo((mid ?? 0) - (high ?? 0), -1);
	});

	// Kills tick labels lost to uPlot's default filter, which keeps only powers of ten on a custom scale.
	test("labels every y tick", async () => {
		const drawn: string[] = [];
		const fillText = CanvasRenderingContext2D.prototype.fillText;
		CanvasRenderingContext2D.prototype.fillText = function (
			this: CanvasRenderingContext2D,
			...args: Parameters<typeof fillText>
		) {
			drawn.push(String(args[0]));
			return fillText.apply(this, args);
		};
		try {
			mount({ data: FLAT });
			await settle();
		} finally {
			CanvasRenderingContext2D.prototype.fillText = fillText;
		}
		expect(drawn).toEqual(
			expect.arrayContaining(["10", "15", "20", "25", "30"]),
		);
		expect(drawn).not.toContain("0");
	});

	// Kills an axis sized only from every line, whose labels clip once hiding lines zooms it to a narrow range.
	test("widens the y axis to fit the labels a narrowed range draws", async () => {
		const data = testData([
			testLine({ id: "near", benchmark: "crc32c", y: [100.1, 100.5, 100.3] }),
			testLine({ id: "far", benchmark: "sha256", y: [500, 500, 500] }),
		]);
		mount({ data });
		await settle();
		const drawn: { text: string; left: number }[] = [];
		const fillText = CanvasRenderingContext2D.prototype.fillText;
		CanvasRenderingContext2D.prototype.fillText = function (
			this: CanvasRenderingContext2D,
			...args: Parameters<typeof fillText>
		) {
			const [text, x] = args;
			if (this.textAlign === "right") {
				drawn.push({ text, left: x - this.measureText(text).width });
			}
			return fillText.apply(this, args);
		};
		try {
			await page.getByRole("button", { name: "Hide sha256" }).click();
			await settle();
		} finally {
			CanvasRenderingContext2D.prototype.fillText = fillText;
		}
		expect(drawn.some(({ text }) => text.startsWith("100."))).toBe(true);
		for (const { text, left } of drawn) {
			expect(left, text).toBeGreaterThanOrEqual(0);
		}
	});

	// Kills stacked plots whose axes size apart, so their x stops lining up once one axis widens.
	test("keeps stacked plots lined up when one axis widens", async () => {
		const data = testData(
			[
				testLine({ id: "near", benchmark: "crc32c", y: [100.1, 100.5, 100.3] }),
				testLine({ id: "far", benchmark: "sha256", y: [500, 500, 500] }),
				testLine({ id: "thr", measure: 1, y: [1, 2, 3] }),
			],
			[
				{ name: "Latency", units: "nanoseconds (ns)" },
				{ name: "Throughput", units: "operations (ops)" },
			],
		);
		mount({ data, layout: "stacked" });
		await settle();
		const before = (await plotArea("2 lines, Latency")).box.left;

		await page.getByRole("button", { name: /^Hide sha256/ }).click();
		await settle();

		const latency = await plotArea("1 line, Latency");
		const throughput = await plotArea("1 line, Throughput");
		expect(latency.box.left).toBeGreaterThan(before);
		expect(throughput.box.left).toBeCloseTo(latency.box.left, 0);
	});

	// Kills a key toggle that does not reach the canvas, or a button that keeps its name.
	test("hides a line from the key and says it can show it again", async () => {
		const { hidden } = mount({ data: FLAT });
		await settle();
		const { canvas, box } = await plotArea();
		const column = box.left + box.width / 2;
		expect(rowsOf(canvas, column, box.top, box.bottom, series(2))).not.toEqual(
			[],
		);

		await page.getByRole("button", { name: "Hide sha256" }).click();
		await settle();

		expect(hidden()).toEqual(["mid"]);
		expect(rowsOf(canvas, column, box.top, box.bottom, series(2))).toEqual([]);
		await expect
			.element(page.getByRole("button", { name: "Show sha256" }))
			.toBeInTheDocument();
		await expect
			.element(page.getByRole("button", { name: "Hide sha256" }))
			.not.toBeInTheDocument();
		await expect.element(page.getByText("1 hidden")).toBeInTheDocument();
	});

	// Kills a band on the wrong side, a band for the wrong line, or no dimming.
	test("focusing a line draws its band on the guarded side and dims the rest", async () => {
		const data = testData([
			testLine({
				id: "guarded",
				benchmark: "blake3",
				y: [10, 10, 10, 10, 10],
				upper: [15, 15, 15, 15, 15],
			}),
			testLine({ id: "other", benchmark: "sha256", y: [30, 30, 30, 30, 30] }),
		]);
		const { focused } = mount({ data, focused: null });
		await settle();
		const { canvas, box } = await plotArea();
		const column = box.left + box.width / 2;
		// A dimmed line is thinner, so none of its pixels is fully covered.
		const dim = tokenRgb("--color-data-dim");
		expect(rowsOf(canvas, column, box.top, box.bottom, dim, 100)).toEqual([]);
		const isBand = (y: number) => {
			const pixel = pixelAt(canvas, column, y);
			return near(pixel.rgb, series(1)) && Math.abs(pixel.alpha - 41) < 14;
		};
		const bandRows = (from: number) =>
			[0, 1, 2, 3, 4, 5].filter((offset) => isBand(from + offset)).length;
		expect(bandRows(box.bottom - 7)).toBe(0);

		await page
			.getByRole("button", {
				name: "Focus blake3 and draw its boundary",
			})
			.click();
		await settle();

		expect(focused()).toBe("guarded");
		expect(bandRows(box.bottom - 7)).toBeGreaterThanOrEqual(3);
		expect(bandRows(box.top + 1)).toBe(0);
		expect(rowsOf(canvas, column, box.top, box.bottom, series(2))).toEqual([]);
		expect(rowsOf(canvas, column, box.top, box.bottom, dim, 100)).not.toEqual(
			[],
		);
		await expect
			.element(
				page.getByRole("button", {
					name: "Focus blake3 and draw its boundary",
				}),
			)
			.toHaveAttribute("aria-pressed", "true");
	});

	test("a lower limit's band runs to the top", async () => {
		const data = testData([
			testLine({
				id: "floor",
				y: [30, 30, 30, 30, 30],
				lower: [25, 25, 25, 25, 25],
			}),
		]);
		mount({ data });
		await settle();
		const { canvas, box } = await plotArea();
		const column = box.left + box.width / 2;
		const isBand = (y: number) => {
			const pixel = pixelAt(canvas, column, y);
			return near(pixel.rgb, series(1)) && Math.abs(pixel.alpha - 41) < 14;
		};
		const top = [1, 2, 3, 4, 5, 6].filter((offset) => isBand(box.top + offset));
		const bottom = [1, 2, 3, 4, 5, 6].filter((offset) =>
			isBand(box.bottom - offset),
		);
		expect(top.length).toBeGreaterThanOrEqual(3);
		expect(bottom).toEqual([]);
	});

	// Kills a hover focus that never reaches the band: the key entry's line draws its boundary while hovered.
	test("hovering a key entry focuses its line until the pointer leaves", async () => {
		const data = testData([
			testLine({
				id: "guarded",
				benchmark: "blake3",
				y: [10, 10, 10, 10, 10],
				upper: [15, 15, 15, 15, 15],
			}),
			testLine({ id: "other", benchmark: "sha256", y: [30, 30, 30, 30, 30] }),
		]);
		const { focused } = mount({ data, focused: null });
		await settle();
		const { canvas, box } = await plotArea();
		const column = box.left + box.width / 2;
		const bandAt = () => {
			const pixel = pixelAt(canvas, column, box.bottom - 3);
			return near(pixel.rgb, series(1)) && pixel.alpha > 20;
		};
		expect(bandAt()).toBe(false);

		await page.getByRole("button", { name: "Hide blake3" }).hover();
		await settle();
		expect(bandAt()).toBe(true);
		expect(focused()).toBeNull();

		await page.getByRole("button", { name: "Hide sha256" }).hover();
		await settle();
		expect(bandAt()).toBe(false);
	});

	// Kills a readout at the wrong index, in raw units, or without its report, or one that announces every hover step.
	test("hovering a point's x reads out its value and report", async () => {
		const data = testData([
			testLine({
				id: "rising",
				y: [10_000, 20_000, 30_000, 40_000, 50_000],
			}),
		]);
		mount({ data, reportHref: (uuid) => `/reports/${uuid}` });
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");

		await page.elementLocator(over).hover({
			position: { x: box.width / 2, y: box.height / 2 },
		});

		await expect.element(status).toHaveTextContent("30.00 µs");
		await expect.element(status).toHaveAttribute("aria-live", "off");
		await expect
			.element(page.getByRole("link", { name: /aaaaaa2/ }))
			.toHaveAttribute("href", "/reports/report-2");

		await page.elementLocator(over).hover({
			position: { x: box.width - 2, y: box.height / 2 },
		});
		await expect.element(status).toHaveTextContent("50.00 µs");

		await page.getByRole("button", { name: "Hide blake3" }).hover();
		await expect.element(status).not.toHaveTextContent("µs");
	});

	// Kills a click that uPlot swallows, which leaves the readout's report link out of reach.
	test("a click pins the readout so its link can be followed", async () => {
		const data = testData([
			testLine({ id: "rising", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
		]);
		mount({ data, reportHref: (uuid) => `/reports/${uuid}` });
		await settle();
		const { over, box } = await plotArea();
		await page.elementLocator(over).click({
			position: { x: box.width / 4, y: box.height / 2 },
		});

		await page.getByRole("link", { name: /aaaaaa1/ }).hover();
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("20.00 µs");
		await expect
			.element(page.getByRole("status"))
			.toHaveAttribute("aria-live", "polite");
		await expect
			.element(page.getByRole("button", { name: "Close the readout" }))
			.toBeInTheDocument();
	});

	// Kills a tap treated as a hover, which a phone loses the moment the finger lifts.
	test("a tap pins the readout on a phone until it is closed", async () => {
		await page.viewport(375, 812);
		const data = testData([
			testLine({ id: "rising", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
		]);
		mount({ data }, 343);
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");
		const at = {
			clientX: box.left + box.width / 2,
			clientY: box.top + box.height / 2,
			bubbles: true,
			pointerType: "touch",
		};
		over.dispatchEvent(new PointerEvent("pointerdown", at));
		over.dispatchEvent(new PointerEvent("pointerup", at));
		over.dispatchEvent(new PointerEvent("click", at));
		await expect.element(status).toHaveTextContent("30.00 µs");
		await expect.element(status).toHaveAttribute("aria-live", "polite");

		over.dispatchEvent(
			new PointerEvent("pointerleave", { ...at, pointerType: "mouse" }),
		);
		await settle();
		await expect.element(status).toHaveTextContent("30.00 µs");

		await page.getByRole("button", { name: "Close the readout" }).click();
		await expect.element(status).not.toHaveTextContent("µs");
	});

	// Kills a keyboard path that moves the readout without announcing it, or a plot area that hides its keys.
	test("arrow keys step the readout and announce it", async () => {
		const data = testData([
			testLine({ id: "rising", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
		]);
		mount({ data });
		await settle();
		const area = page.getByRole("group", { name: "1 line, Latency" });
		await expect.element(area).toHaveAttribute("aria-roledescription", "plot");
		await expect.element(area).toHaveAccessibleDescription(/Arrow keys/);
		await expect.element(area).toHaveAccessibleDescription(/Escape/);
		(area.element() as HTMLElement).focus();
		const status = page.getByRole("status");

		await userEvent.keyboard("{ArrowRight}");
		await expect.element(status).toHaveTextContent("10.00 µs");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await userEvent.keyboard("{ArrowRight}");
		await expect.element(status).toHaveTextContent("20.00 µs");
		await userEvent.keyboard("{End}");
		await expect.element(status).toHaveTextContent("50.00 µs");
		await userEvent.keyboard("{Home}");
		await expect.element(status).toHaveTextContent("10.00 µs");
		await userEvent.keyboard("{Escape}");
		await expect.element(status).not.toHaveTextContent("µs");
	});

	// Kills a ring drawn at another line's point than the focused one.
	test("rings the hovered line's point", async () => {
		mount({ data: FLAT, focused: null });
		await settle();
		const { over, canvas, box } = await plotArea();
		const column = box.left + box.width / 2;
		const high = middle(rowsOf(canvas, column, box.top, box.bottom, series(3)));
		await page.elementLocator(over).hover({
			position: { x: box.width / 2, y: high - box.top },
		});
		// The ring is decorative, so it has no role to find it by.
		const ring = document.querySelector(".pl-ring")?.getBoundingClientRect();
		expect(ring).toBeDefined();
		expect((ring?.top ?? 0) + (ring?.height ?? 0) / 2).toBeCloseTo(high, -0.5);
	});

	// Kills colors read once at mount, which strand the plot in the old theme.
	test("redraws in the other theme's colors when the theme changes", async () => {
		mount({ data: FLAT });
		await settle();
		const { canvas, box } = await plotArea();
		const column = box.left + box.width / 2;
		const dark = series(1);
		expect(rowsOf(canvas, column, box.top, box.bottom, dark)).not.toEqual([]);

		document.documentElement.setAttribute(THEME_ATTRIBUTE, "light");
		await settle();

		const light = series(1);
		expect(near(dark, light)).toBe(false);
		expect(rowsOf(canvas, column, box.top, box.bottom, light)).not.toEqual([]);
		expect(rowsOf(canvas, column, box.top, box.bottom, dark)).toEqual([]);
	});

	// Kills a stacked layout that shares one plot, or panels whose cursors move apart.
	test("stacks one plot per measure on one x with a synced cursor", async () => {
		const data = testData(
			[
				testLine({ id: "lat", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
				testLine({ id: "thr", measure: 1, y: [1, 2, 3, 4, 5] }),
			],
			[
				{ name: "Latency", units: "nanoseconds (ns)" },
				{ name: "Throughput", units: "operations (ops)" },
			],
		);
		mount({ data, layout: "stacked" });
		await settle();
		const latency = await plotArea("1 line, Latency");
		const throughput = await plotArea("1 line, Throughput");
		expect(throughput.box.top).toBeGreaterThan(latency.box.bottom);
		expect(throughput.box.left).toBeCloseTo(latency.box.left, 0);
		expect(throughput.box.width).toBeCloseTo(latency.box.width, 0);

		await page.elementLocator(latency.over).hover({
			position: { x: latency.box.width / 2, y: latency.box.height / 2 },
		});

		const status = page.getByRole("status");
		await expect.element(status).toHaveTextContent("30.00 µs");
		await expect.element(status).toHaveTextContent("3.00 ops");
		const guide = (over: HTMLElement) =>
			over.querySelector(".u-cursor-x")?.getBoundingClientRect().left;
		expect(guide(throughput.over)).toBeCloseTo(guide(latency.over) ?? 0, 0);
	});

	// Kills a dual axis drawn as two plots, or the dots put on the wrong measure.
	test("draws two measures on one plot, the second dotted", async () => {
		const data = testData(
			[
				// The limit lifts this line above the middle, where the other measure's line sits.
				testLine({
					id: "lat",
					y: [20, 20, 20, 20, 20],
					lower: [10, 10, 10, 10, 10],
				}),
				testLine({
					id: "thr",
					benchmark: "sha256",
					measure: 1,
					y: [3, 3, 3, 3, 3],
				}),
			],
			[
				{ name: "Latency", units: "nanoseconds (ns)" },
				{ name: "Throughput", units: "operations (ops)" },
			],
		);
		mount({ data, focused: null });
		await settle();
		const { canvas, box } = await plotArea("2 lines, Latency and Throughput");
		// Between two points, clear of their marks.
		const column = box.left + box.width / 8;
		// Solid strokes cover their row end to end; dots leave gaps.
		const coverage = (slot: number) => {
			const row = middle(
				rowsOf(canvas, column, box.top, box.bottom, series(slot)),
			);
			const from = Math.ceil(box.left + 20);
			const to = Math.floor(box.right - 20);
			let hits = 0;
			for (let x = from; x < to; x++) {
				const pixel = pixelAt(canvas, x, row);
				if (pixel.alpha > 100 && near(pixel.rgb, series(slot), 30)) {
					hits++;
				}
			}
			return hits / (to - from);
		};
		expect(coverage(1)).toBeGreaterThan(0.95);
		expect(coverage(2)).toBeLessThan(0.8);
		expect(coverage(2)).toBeGreaterThan(0.2);
	});

	// Kills a version axis drawn in time order, where versions from two branches interleave.
	test("orders points by version on a version axis", async () => {
		const data: PlotData = {
			x: [START, START + DAY, START + 2 * DAY],
			report: [0, 1, 2],
			reports: [
				{ uuid: "r0", version: 3 },
				{ uuid: "r1", version: 1 },
				{ uuid: "r2", version: 2 },
			],
			measures: [{ name: "Latency", units: "nanoseconds (ns)" }],
			lines: [testLine({ id: "a", y: [30_000, 10_000, 20_000] })],
		};
		mount({ data, xAxis: "version" });
		await settle();
		const { over, box } = await plotArea();
		await page.elementLocator(over).hover({
			position: { x: 1, y: box.height / 2 },
		});
		const status = page.getByRole("status");
		await expect.element(status).toHaveTextContent("Version 1");
		await expect.element(status).toHaveTextContent("10.00 µs");
	});

	// Kills a frame that grows or moves once the data lands, as a key entry would if a long name wrapped.
	test("holds its frame from the reserved state to the drawn plot", async () => {
		const parameters = (threads: number) => ({
			input_bytes: 1_048_576 + threads,
			simd: `avx${threads}`,
			threads,
		});
		const { root, setData } = mount(
			{
				data: undefined,
				reserve: { lines: 3, measures: 1 },
				note: "4w, ending at this report",
			},
			640,
		);
		await settle();
		const figure = root.querySelector("figure") as HTMLElement;
		const before = figure.getBoundingClientRect();
		expect(page.getByRole("button", { name: /^Hide/ }).query()).toBeNull();

		setData(
			testData(
				[1, 2, 3].map((threads) =>
					testLine({
						id: `${threads}`,
						benchmark: "a_benchmark_with_a_long_name",
						parameters: parameters(threads),
						y: [threads, threads],
					}),
				),
			),
		);
		await settle();
		await plotArea();

		const after = figure.getBoundingClientRect();
		expect(after.height).toBe(before.height);
		expect(after.width).toBe(before.width);
	});

	// Kills a key that never folds, or folds without a way back.
	test("folds the key past twelve lines behind Show all", async () => {
		const data = testData(
			Array.from({ length: 14 }, (_, index) =>
				testLine({
					id: `l${index}`,
					parameters: { threads: index },
					y: [index + 1, index + 1],
				}),
			),
		);
		mount({ data });
		await settle();
		const toggles = () => page.getByRole("button", { name: /^Hide/ }).all();
		expect(toggles()).toHaveLength(12);

		await page.getByRole("button", { name: "Show all 14" }).click();
		expect(toggles()).toHaveLength(14);
		await expect
			.element(page.getByRole("button", { name: "Show fewer" }))
			.toHaveAttribute("aria-expanded", "true");
	});

	test("lists alerting lines first in the key", async () => {
		const data = testData([
			testLine({ id: "quiet", benchmark: "crc32c", y: [1, 1] }),
			testLine({
				id: "loud",
				benchmark: "xxh3",
				alerting: true,
				y: [1, 9],
				upper: [2, 2],
				alerts: [1],
			}),
		]);
		mount({ data, focused: null });
		await settle();
		const names = page
			.getByRole("button", { name: /^Hide/ })
			.all()
			.map((button) => button.element().getAttribute("aria-label"));
		expect(names).toEqual(["Hide xxh3", "Hide crc32c"]);
	});

	// Kills alert markers drawn only for the focused line.
	test("marks every visible line's alerts", async () => {
		const data = testData([
			testLine({
				id: "a",
				benchmark: "blake3",
				alerting: true,
				y: [10, 10, 10, 10, 30],
				upper: [12, 12, 12, 12, 12],
				alerts: [4],
			}),
			testLine({
				id: "b",
				benchmark: "sha256",
				alerting: true,
				y: [40, 40, 40, 40, 60],
				upper: [42, 42, 42, 42, 42],
				alerts: [4],
			}),
		]);
		mount({ data, focused: null });
		await settle();
		const { canvas, box } = await plotArea();
		// Beside the marker's center, clear of its exclamation mark.
		const column = box.right - 4;
		const rows = rowsOf(
			canvas,
			column,
			box.top,
			box.bottom,
			tokenRgb("--color-marker"),
		);
		const markers = rows.filter(
			(row, index) => index === 0 || row - (rows[index - 1] ?? 0) > 1,
		);
		expect(markers).toHaveLength(2);
	});
});

// Six lines with long names, so a pinned readout is wider than half a phone's plot.
const WIDE = testData(
	Array.from({ length: 6 }, (_, index) =>
		testLine({
			id: `wide-${index}`,
			benchmark: "a_benchmark_with_a_long_name",
			parameters: { threads: index, simd: "avx512" },
			y: [1, 2, 3, 4, 5].map((point) => 1000 * point + index * 100),
		}),
	),
);

const readoutBox = () => {
	// The readout card has no role of its own; its live region holds it and the ring.
	const card = page.getByRole("status").element().querySelector(".pl-readout");
	if (!card) {
		throw new Error("No readout");
	}
	return card.getBoundingClientRect();
};

const bodyBox = (over: HTMLElement) => {
	const body = over.closest(".pl-body");
	if (!body) {
		throw new Error("No plot body");
	}
	return body.getBoundingClientRect();
};

describe("Plot readout room", () => {
	// Kills a readout placed by the cursor's half of the plot, which runs off a phone's screen and hides its close button.
	test("keeps a tapped readout inside a phone's plot", async () => {
		await page.viewport(414, 896);
		mount({ data: WIDE }, 382);
		await settle();
		const { over, box } = await plotArea();
		for (const fraction of [0.1, 0.45, 0.55, 0.95]) {
			const at = {
				clientX: box.left + box.width * fraction,
				clientY: box.top + box.height / 2,
				bubbles: true,
				pointerType: "touch",
			};
			over.dispatchEvent(new PointerEvent("click", at));
			await settle();
			const readout = readoutBox();
			const body = bodyBox(over);
			expect(readout.left, `${fraction}`).toBeGreaterThanOrEqual(body.left);
			expect(readout.right, `${fraction}`).toBeLessThanOrEqual(body.right);
			const close = page
				.getByRole("button", { name: "Close the readout" })
				.element()
				.getBoundingClientRect();
			expect(close.left, `${fraction}`).toBeGreaterThanOrEqual(0);
			expect(close.right, `${fraction}`).toBeLessThanOrEqual(innerWidth);
			expect(document.documentElement.scrollWidth).toBe(innerWidth);
		}
	});

	test("keeps a hovered readout inside a 400 px tile", async () => {
		mount({ data: WIDE, size: "tile" }, 400);
		await settle();
		const { over, box } = await plotArea();
		for (const fraction of [0.1, 0.45, 0.55, 0.95]) {
			await page.elementLocator(over).hover({
				position: { x: box.width * fraction, y: box.height / 2 },
			});
			await settle();
			const readout = readoutBox();
			const body = bodyBox(over);
			expect(readout.left, `${fraction}`).toBeGreaterThanOrEqual(body.left);
			expect(readout.right, `${fraction}`).toBeLessThanOrEqual(body.right);
		}
	});
});

describe("Plot on a phone", () => {
	// Kills a narrow signal that is stale at first use, which leaves a phone's first render at desktop sizes.
	// It holds only because the viewport changes while nothing is mounted: beforeEach sets 1280 before this test sets 414.
	test("folds the key after six entries", async () => {
		await page.viewport(414, 896);
		const data = testData(
			Array.from({ length: 8 }, (_, index) =>
				testLine({
					id: `l${index}`,
					parameters: { threads: index },
					y: [index + 1, index + 1],
				}),
			),
		);
		mount({ data }, 382);
		await settle();
		expect(page.getByRole("button", { name: /^Hide/ }).all()).toHaveLength(6);
		await expect
			.element(page.getByRole("button", { name: "Show all 8" }))
			.toBeInTheDocument();
	});
});

describe("Plot axes", () => {
	// Kills a right axis that ignores the labels it draws, which clip at the canvas's right edge.
	test("widens the right axis to fit the labels a narrowed range draws", async () => {
		const data = testData(
			[
				testLine({ id: "lat", y: [10, 20, 30] }),
				testLine({
					id: "near",
					benchmark: "crc32c",
					measure: 1,
					y: [100.1, 100.5, 100.3],
				}),
				testLine({
					id: "far",
					benchmark: "sha256",
					measure: 1,
					y: [500, 500, 500],
				}),
			],
			[
				{ name: "Latency", units: "nanoseconds (ns)" },
				{ name: "Throughput", units: "operations (ops)" },
			],
		);
		mount({ data });
		await settle();
		const { canvas } = await plotArea("3 lines, Latency and Throughput");
		const drawn: { text: string; right: number }[] = [];
		const fillText = CanvasRenderingContext2D.prototype.fillText;
		CanvasRenderingContext2D.prototype.fillText = function (
			this: CanvasRenderingContext2D,
			...args: Parameters<typeof fillText>
		) {
			const [text, x] = args;
			if (this.textAlign === "left") {
				drawn.push({ text, right: x + this.measureText(text).width });
			}
			return fillText.apply(this, args);
		};
		try {
			await page.getByRole("button", { name: /^Hide sha256/ }).click();
			await settle();
		} finally {
			CanvasRenderingContext2D.prototype.fillText = fillText;
		}
		expect(drawn.some(({ text }) => text.startsWith("100."))).toBe(true);
		for (const { text, right } of drawn) {
			expect(right, text).toBeLessThanOrEqual(canvas.width);
		}
	});

	// Kills an axis that shrinks to the visible lines' labels, which moves the plot area under the reader.
	test("never narrows the y axis below the width every line needs", async () => {
		const data = testData([
			testLine({ id: "small", benchmark: "crc32c", y: [1, 2, 3] }),
			testLine({ id: "big", benchmark: "sha256", y: [50_000, 50_000, 50_000] }),
		]);
		mount({ data });
		await settle();
		const before = (await plotArea()).box.left;

		await page.getByRole("button", { name: "Hide sha256" }).click();
		await settle();

		expect((await plotArea()).box.left).toBe(before);
	});
});

describe("Plot cleanup", () => {
	// Kills a plot that keeps its sync key or its media query listener after it unmounts.
	test("returns its sync key and its media listener when it unmounts", async () => {
		const removed = vi.spyOn(MediaQueryList.prototype, "removeEventListener");
		const data = testData(
			[
				testLine({ id: "lat", y: [1, 2, 3] }),
				testLine({ id: "thr", measure: 1, y: [3, 2, 1] }),
			],
			[
				{ name: "Latency", units: "nanoseconds (ns)" },
				{ name: "Throughput", units: "operations (ops)" },
			],
		);
		try {
			for (const round of [1, 2]) {
				const key = takeSyncKey();
				returnSyncKey(key);
				mount({ data, layout: "stacked" });
				await settle();
				dispose?.();
				dispose = undefined;
				const next = takeSyncKey();
				returnSyncKey(next);
				expect(next, `round ${round}`).toBe(key);
				expect(
					removed.mock.calls.filter(([type]) => type === "change"),
					`round ${round}`,
				).toHaveLength(round);
			}
		} finally {
			removed.mockRestore();
		}
	});
});

describe("Plot canvas state", () => {
	// Kills overlay marks that leave the canvas's style behind, which uPlot's style cache then draws the grid with.
	test("draws the grid the same on every frame", async () => {
		const data = testData(
			Array.from({ length: 10 }, (_, index) =>
				testLine({
					id: `l${index}`,
					parameters: { threads: index },
					y: [1, 2, 3, 2, 1].map((point) => point * 10 + index),
				}),
			),
		);
		// Past eight series the ninth line's points are hollow marks, drawn with their own stroke width.
		mount({ data, focused: "l8" });
		await settle();
		const { canvas, box } = await plotArea();
		const column = box.left + box.width / 10;
		const shot = () =>
			Array.from({ length: Math.floor(box.height) }, (_, row) => {
				const { rgb, alpha } = pixelAt(canvas, column, box.top + row);
				return [...rgb, alpha].join(",");
			});
		const first = shot();

		document.documentElement.setAttribute(THEME_ATTRIBUTE, "light");
		await settle();
		document.documentElement.setAttribute(THEME_ATTRIBUTE, "dark");
		await settle();

		expect(shot()).toEqual(first);
	});
});
