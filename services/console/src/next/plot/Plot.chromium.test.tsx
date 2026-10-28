import "@bencherdev/ui/styles.css";
import "./plot.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import Plot, { type PlotAlerts, type PlotProps } from "./Plot";
import { middle, near, pixelAt, rowsOf, settle, tokenRgb } from "./pixels";
import { returnSyncKey, takeSyncKey } from "./sync";
import { expectNameStartsWithText, shownText } from "../ui/testing";
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
			await page.getByRole("button", { name: "sha256: hide its line" }).click();
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

		await page
			.getByRole("button", { name: /^sha256.*: hide its line$/ })
			.click();
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

		await page.getByRole("button", { name: "sha256: hide its line" }).click();
		await settle();

		expect(hidden()).toEqual(["mid"]);
		expect(rowsOf(canvas, column, box.top, box.bottom, series(2))).toEqual([]);
		await expect
			.element(page.getByRole("button", { name: "sha256: show its line" }))
			.toBeInTheDocument();
		await expect
			.element(page.getByRole("button", { name: "sha256: hide its line" }))
			.not.toBeInTheDocument();
		await expect.element(page.getByText("1 hidden")).toBeInTheDocument();
	});

	// Kills a band on the wrong side, a band for the wrong line, or no dimming, and a focus button that keeps its name once its line is focused or that cannot clear it.
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

		await page.getByRole("button", { name: "Focus line of blake3" }).click();
		await settle();

		expect(focused()).toBe("guarded");
		expect(bandRows(box.bottom - 7)).toBeGreaterThanOrEqual(3);
		expect(bandRows(box.top + 1)).toBe(0);
		expect(rowsOf(canvas, column, box.top, box.bottom, series(2))).toEqual([]);
		expect(rowsOf(canvas, column, box.top, box.bottom, dim, 100)).not.toEqual(
			[],
		);
		await expect
			.element(page.getByRole("button", { name: "sha256: hide its line" }))
			.toBeInTheDocument();
		await page.getByRole("button", { name: "Clear focus of blake3" }).click();
		expect(focused()).toBeNull();
		await expect
			.element(page.getByRole("button", { name: "Focus line of blake3" }))
			.toBeInTheDocument();
	});

	// Kills a focus target only on lines with a boundary, which leaves a line focused by a link with no way to clear it.
	test("a line with no boundary can be focused and cleared from its key entry", async () => {
		const data = testData([
			testLine({ id: "one", benchmark: "blake3", y: [10, 10, 10, 10, 10] }),
			testLine({ id: "other", benchmark: "sha256", y: [30, 30, 30, 30, 30] }),
		]);
		const { focused } = mount({ data, focused: null });
		await settle();
		const focus = page.getByRole("button", { name: "Focus line of sha256" });
		expect(focus.query()).not.toBeNull();

		await focus.click();
		expect(focused()).toBe("other");
		await page.getByRole("button", { name: "Clear focus of sha256" }).click();
		expect(focused()).toBeNull();
	});

	// Kills a click on a hidden focused line, which reads "Focus line", that clears its focus once the host shows it at once.
	test("focusing a hidden focused line shows it and keeps it focused", async () => {
		const data = testData([
			testLine({ id: "one", benchmark: "blake3", y: [10, 10, 10, 10, 10] }),
			testLine({ id: "other", benchmark: "sha256", y: [30, 30, 30, 30, 30] }),
		]);
		const { focused, hidden } = mount({
			data,
			focused: "one",
			hidden: ["one"],
		});
		await settle();

		await page.getByRole("button", { name: "Focus line of blake3" }).click();
		expect(focused()).toBe("one");
		expect(hidden()).toEqual([]);
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

		await page.getByRole("button", { name: "blake3: hide its line" }).hover();
		await settle();
		expect(bandAt()).toBe(true);
		expect(focused()).toBeNull();

		await page.getByRole("button", { name: "sha256: hide its line" }).hover();
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
		await expect.element(status).toHaveTextContent("aaaaaa2");
		await expect.element(status).toHaveAttribute("aria-live", "off");

		await page.elementLocator(over).hover({
			position: { x: box.width - 2, y: box.height / 2 },
		});
		await expect.element(status).toHaveTextContent("50.00 µs");

		await page.getByRole("button", { name: "blake3: hide its line" }).hover();
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

		const link = page.getByRole("link", { name: /aaaaaa1/ });
		await expect.element(link).toHaveAttribute("href", "/reports/report-1");
		await link.hover();
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

	// Kills an unpinned readout that takes the pointer or offers a link or a button, which keeps a mouse from the points beneath it, and a pinned readout the pointer passes through, whose link no mouse can reach.
	test("an unpinned readout lets the pointer through to the plot, and a pinned one takes it", async () => {
		const data = testData([
			testLine({ id: "rising", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
		]);
		mount({ data, reportHref: (uuid) => `/reports/${uuid}` });
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");
		const hit = (x: number, y: number) =>
			status
				.element()
				.querySelector(".pl-readout")
				?.contains(document.elementFromPoint(x, y));
		await page.elementLocator(over).hover({
			position: { x: box.width / 4, y: box.height / 2 },
		});
		await expect.element(status).toHaveTextContent("20.00 µs");
		expect(status.element().querySelector("a, button")).toBeNull();
		// The readout's far edge is nearer the next point than its own.
		const card = readoutBox();
		const edge = { x: card.right - 4, y: card.top + card.height / 2 };
		expect(hit(edge.x, edge.y)).toBe(false);
		await page.elementLocator(over).hover({
			position: { x: edge.x - box.left, y: edge.y - box.top },
		});
		await expect.element(status).toHaveTextContent("30.00 µs");

		await page.elementLocator(over).click({
			position: { x: box.width / 4, y: box.height / 2 },
		});
		await expect.element(status).toHaveTextContent("20.00 µs");
		const pinned = readoutBox();
		expect(hit(pinned.right - 4, pinned.top + pinned.height / 2)).toBe(true);
		const link = page
			.getByRole("link", { name: /aaaaaa1/ })
			.element()
			.getBoundingClientRect();
		expect(
			document
				.elementFromPoint(
					link.left + link.width / 2,
					link.top + link.height / 2,
				)
				?.closest("a")
				?.getAttribute("href"),
		).toBe("/reports/report-1");
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

	// Kills a click on the pinned point that keeps it pinned or closes a mouse's readout, a click on another point that unpins instead of moving the pin, and a readout that keeps "Click to pin" once pinned or leaves it out while it follows the mouse.
	test("a click on the pinned point unpins it, and a click on another point moves the pin there", async () => {
		const data = testData([
			testLine({ id: "rising", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
		]);
		mount({ data });
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");
		const close = page.getByRole("button", { name: "Close the readout" });
		const hint = page.getByText("Click to pin");
		const at = (fraction: number) => ({
			position: { x: box.width * fraction, y: box.height / 2 },
		});

		await page.elementLocator(over).hover(at(0.25));
		await expect.element(status).toHaveTextContent("20.00 µs");
		await expect.element(hint).toBeInTheDocument();
		await expect.element(close).not.toBeInTheDocument();

		await page.elementLocator(over).click(at(0.25));
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await expect.element(close).toBeInTheDocument();
		await expect.element(hint).not.toBeInTheDocument();

		await page.elementLocator(over).click(at(0.25));
		await expect.element(status).toHaveAttribute("aria-live", "off");
		await expect.element(status).toHaveTextContent("20.00 µs");
		await expect.element(close).not.toBeInTheDocument();
		await expect.element(hint).toBeInTheDocument();

		await page.elementLocator(over).click(at(0.25));
		await page.elementLocator(over).click(at(0.75));
		await expect.element(status).toHaveTextContent("40.00 µs");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
	});

	// Kills a tap on the pinned point that keeps it pinned, leaves a readout no finger can move, or is told from its click rather than its press, and a tap elsewhere that does not move the pin.
	test("a tap on the pinned point closes the readout, and a tap elsewhere moves it", async () => {
		await page.viewport(375, 812);
		const data = testData([
			testLine({ id: "rising", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
		]);
		mount({ data }, 343);
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");
		// A finger's press, then a click that some browsers send as a plain mouse event.
		const tap = (fraction: number) => {
			const at = {
				clientX: box.left + box.width * fraction,
				clientY: box.top + box.height / 2,
				bubbles: true,
			};
			over.dispatchEvent(
				new PointerEvent("pointerdown", { ...at, pointerType: "touch" }),
			);
			over.dispatchEvent(new MouseEvent("click", at));
		};
		tap(0.25);
		await expect.element(status).toHaveTextContent("20.00 µs");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		tap(0.75);
		await expect.element(status).toHaveTextContent("40.00 µs");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		tap(0.75);
		await expect.element(status).not.toHaveTextContent("µs");
	});

	// Kills Enter or Space that does nothing, only pins, scrolls the page, or holds the page's Space with nothing to pin, an unpin that loses the point, and a readout the keys unpinned that offers a click.
	test("Enter or Space pins the cursor's point and unpins it", async () => {
		const data = testData([
			testLine({ id: "rising", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
		]);
		mount({ data });
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");
		const space = () =>
			over.dispatchEvent(
				new KeyboardEvent("keydown", {
					key: " ",
					bubbles: true,
					cancelable: true,
				}),
			);
		over.focus();
		expect(space()).toBe(true);
		await page.elementLocator(over).hover({
			position: { x: box.width / 4, y: box.height / 2 },
		});
		await expect.element(status).toHaveAttribute("aria-live", "off");

		await userEvent.keyboard("{Enter}");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await expect.element(status).toHaveTextContent("20.00 µs");
		expect(space()).toBe(false);
		await expect.element(status).toHaveAttribute("aria-live", "off");
		await expect.element(status).toHaveTextContent("20.00 µs");
		await expect
			.element(page.getByText("Click to pin"))
			.not.toBeInTheDocument();
		await userEvent.keyboard("{Enter}");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await expect.element(status).toHaveTextContent("20.00 µs");
	});

	// Kills a pin from the keys that a click on a line's point at its x moves to that line, so unpinning takes two clicks.
	test("one click on its point unpins a readout the keys pinned", async () => {
		mount({ data: FLAT, focused: null });
		await settle();
		const { over, canvas, box } = await plotArea();
		over.focus();
		await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
		const status = page.getByRole("status");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		const column = box.left + box.width / 2;
		const low = middle(rowsOf(canvas, column, box.top, box.bottom, series(1)));
		await page.elementLocator(over).click({
			position: { x: box.width / 2, y: low - box.top },
		});
		await expect.element(status).toHaveAttribute("aria-live", "off");
	});

	// Kills a hint the keys turned off that stays off for a mouse, after a hover or after a click that unpins a pin the keys made.
	test("a mouse gets Click to pin back after the keys moved or pinned the readout", async () => {
		const data = testData([
			testLine({ id: "rising", y: [10_000, 20_000, 30_000, 40_000, 50_000] }),
		]);
		mount({ data });
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");
		const hint = page.getByText("Click to pin");
		over.focus();
		await userEvent.keyboard("{ArrowRight}{ArrowRight}{Enter}");
		await expect.element(status).toHaveAttribute("aria-live", "off");
		await expect.element(hint).not.toBeInTheDocument();
		await page.elementLocator(over).hover({
			position: { x: box.width / 4, y: box.height / 2 },
		});
		await expect.element(hint).toBeInTheDocument();

		await userEvent.keyboard("{ArrowRight}");
		await expect.element(status).toHaveTextContent("30.00 µs");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await page.elementLocator(over).click({
			position: { x: box.width / 2, y: box.height / 2 },
		});
		await expect.element(status).toHaveAttribute("aria-live", "off");
		await expect.element(hint).toBeInTheDocument();
	});

	// Kills a mouse unpin off every line that keeps the line it pinned on focused in the readout.
	test("a click off every line that unpins a point lets its line go", async () => {
		mount({ data: FLAT, focused: null });
		await settle();
		const { over, canvas, box } = await plotArea();
		const column = box.left + box.width / 2;
		const low = middle(rowsOf(canvas, column, box.top, box.bottom, series(1)));
		const mid = middle(rowsOf(canvas, column, box.top, box.bottom, series(2)));
		const focusedName = () =>
			page
				.getByRole("status")
				.element()
				.querySelector(".pl-rrow.focused .pl-rname")?.textContent;
		await page.elementLocator(over).click({
			position: { x: box.width / 2, y: low - box.top },
		});
		await expect.poll(focusedName).toBe("crc32c");
		await page.elementLocator(over).click({
			position: { x: box.width / 2, y: (low + mid) / 2 - box.top },
		});
		await expect
			.element(page.getByRole("status"))
			.toHaveAttribute("aria-live", "off");
		expect(focusedName()).toBeUndefined();
	});

	// Kills a pinned point told by its index, where several reports share its x on a version axis, so a click on its other side pins it again.
	test("a click on either side of a pinned point shared by several reports unpins it", async () => {
		const data: PlotData = {
			x: [
				START,
				START + DAY,
				START + 2 * DAY,
				START + 3 * DAY,
				START + 4 * DAY,
			],
			report: [0, 1, 2, 3, 4],
			reports: [1, 2, 2, 2, 3].map((version, index) => ({
				uuid: `report-${index}`,
				version,
				hash: `aaaaaa${index}`,
			})),
			measures: [{ name: "Latency", units: "nanoseconds (ns)" }],
			lines: [testLine({ id: "a", y: [10, 11, 12, 13, 14] })],
		};
		mount({ data, xAxis: "version" });
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");
		const at = (dx: number) => ({
			position: { x: box.width / 2 + dx, y: box.height / 2 },
		});
		await page.elementLocator(over).click(at(-3));
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await page.elementLocator(over).click(at(3));
		await expect.element(status).toHaveAttribute("aria-live", "off");
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
		expect(
			page.getByRole("button", { name: /: hide its line$/ }).query(),
		).toBeNull();

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
		const toggles = () =>
			page.getByRole("button", { name: /: hide its line$/ }).all();
		expect(toggles()).toHaveLength(12);

		await page.getByRole("button", { name: "Show all 14" }).click();
		expect(toggles()).toHaveLength(14);
		await expect
			.element(page.getByRole("button", { name: "Show fewer" }))
			.toHaveAttribute("aria-expanded", "true");
	});

	// Kills a toggle named "Hide hash size=1" over "hash size=1", or tags run
	// into the benchmark ("hashsize=1"), so voice control cannot reach it.
	test("names each key toggle first by the line it shows, shown or hidden", async () => {
		const data = testData([
			testLine({
				id: "one",
				benchmark: "hash",
				parameters: { size: 1 },
				y: [1, 2],
			}),
			testLine({
				id: "two",
				benchmark: "hash",
				parameters: { size: 2 },
				testbed: "macos",
				y: [2, 3],
			}),
		]);
		mount({ data, focused: null });
		await settle();
		const toggles = page.getByRole("button", { name: /: hide its line$/ });
		expect(toggles.all()).toHaveLength(2);
		for (const toggle of toggles.all()) {
			expectNameStartsWithText(toggle.element());
		}
		expect(shownText(toggles.first().element())).toBe("hash size=1 linux");
		await toggles.first().click();
		const shown = page.getByRole("button", { name: /: show its line$/ });
		await expect.element(shown).toBeVisible();
		expectNameStartsWithText(shown.element());
	});

	// Kills a key control or the shared parameters losing the tooltip that says what they are, or the keyboard's way to it.
	test("describes each key control and the shared parameters in a tooltip", async () => {
		const data = testData([
			testLine({
				id: "one",
				parameters: { simd: "avx2", threads: 1 },
				y: [1, 2],
			}),
			testLine({
				id: "two",
				parameters: { simd: "avx2", threads: 2 },
				y: [2, 3],
			}),
		]);
		mount({ data, focused: null });
		await settle();
		const controls = [
			page.getByRole("button", { name: "blake3 threads=1: hide its line" }),
			page.getByRole("button", { name: "Focus line of blake3 threads=1" }),
		];
		for (const control of controls) {
			await expect.element(control).toHaveAccessibleDescription(/\S/);
			await control.hover();
			const tip = document.getElementById(
				control.element().getAttribute("aria-describedby") ?? "",
			);
			expect(tip?.matches(":popover-open")).toBe(true);
		}
		const shared = page.getByText("simd=avx2", { exact: true });
		await expect.element(shared).toHaveAccessibleDescription(/\S/);
		(shared.element() as HTMLElement).focus();
		expect(document.activeElement).toBe(shared.element());
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
			.getByRole("button", { name: /: hide its line$/ })
			.all()
			.map((button) => button.element().getAttribute("aria-label"));
		expect(names).toEqual(["xxh3: hide its line", "crc32c: hide its line"]);
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

// One line whose middle point alerted, as an alert's page and Explore draw it.
const ALERTED = testData([
	testLine({
		id: "alerted",
		benchmark: "blake3",
		parameters: { threads: 1, simd: "avx2" },
		alerting: true,
		y: [10_000, 10_000, 30_000, 10_000, 10_000],
		upper: [12_000, 12_000, 12_000, 12_000, 12_000],
		alerts: [2],
		raised: [{ index: 2, uuid: "alert-2", status: "active" }],
	}),
]);
const ALERT_NAME = "blake3 simd=avx2 threads=1 Latency";

/** Whether the axes row counts an alert and the key marks a line alerting. */
const alertMarks = () => ({
	count: document.querySelector("figure .pl-pill.alert") !== null,
	dot: document.querySelector(".pl-key .pl-alertdot") !== null,
});

/** The rows of `color` down the column just right of the alerted point's center. */
const markerRows = async (token: string) => {
	const { canvas, box } = await plotArea();
	return rowsOf(
		canvas,
		box.left + box.width / 2 + 4,
		box.top,
		box.bottom,
		tokenRgb(token),
	);
};

describe("Plot alerts", () => {
	// Kills alert controls on an unpinned readout or another point, a link to
	// another alert, a change of the wrong alert or to the wrong status, a
	// button that is replaced rather than relabeled (dropping the focus), and a
	// marker, the axes row's alert count, or the key's alert dot that keeps its
	// place when the alert's status changes.
	test("a pinned readout on an alert links to it and changes it, and its marker follows", async () => {
		const [status, setStatus] = createSignal<"active" | "dismissed">();
		const changes: [string, string][] = [];
		mount({
			data: ALERTED,
			alerts: {
				href: (uuid) => `/alerts/${uuid}`,
				status: () => status(),
				change: (uuid) => (next) => {
					changes.push([uuid, next]);
					setStatus(next);
				},
			},
		});
		await settle();
		const { over, box } = await plotArea();
		const at = (fraction: number) => ({
			position: { x: box.width * fraction, y: box.height / 2 },
		});
		const link = page.getByRole("link", { name: `Alert: ${ALERT_NAME}` });
		const dismiss = page.getByRole("button", {
			name: `Dismiss the alert on ${ALERT_NAME}`,
		});
		const reactivate = page.getByRole("button", {
			name: `Reactivate the alert on ${ALERT_NAME}`,
		});
		expect(await markerRows("--color-marker")).not.toHaveLength(0);
		expect(alertMarks()).toEqual({ count: true, dot: true });

		await page.elementLocator(over).hover(at(0.5));
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("30.00 µs");
		await expect.element(link).not.toBeInTheDocument();

		await page.elementLocator(over).click(at(0.5));
		await expect.element(link).toHaveAttribute("href", "/alerts/alert-2");
		expectNameStartsWithText(link.element());
		expectNameStartsWithText(dismiss.element());
		await dismiss.click();
		expect(changes).toEqual([["alert-2", "dismissed"]]);
		await expect.element(reactivate).toHaveFocus();
		await settle();
		expect(await markerRows("--color-marker")).toHaveLength(0);
		expect(await markerRows("--color-text-muted")).not.toHaveLength(0);
		expect(alertMarks()).toEqual({ count: false, dot: false });

		await reactivate.click();
		expect(changes).toEqual([
			["alert-2", "dismissed"],
			["alert-2", "active"],
		]);
		await expect.element(dismiss).toHaveFocus();
		await settle();
		expect(await markerRows("--color-marker")).not.toHaveLength(0);
		expect(alertMarks()).toEqual({ count: true, dot: true });

		await page.elementLocator(over).click(at(0.25));
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("10.00 µs");
		await expect.element(link).not.toBeInTheDocument();
		await expect.element(dismiss).not.toBeInTheDocument();
	});

	// Kills a marker drawn by the alert's status alone, which a dismissed alert's page draws red.
	test("draws a marker muted while the data says its alert is no longer active", async () => {
		mount({
			data: {
				...ALERTED,
				lines: ALERTED.lines.map((line) => ({
					...line,
					raised: [{ index: 2, uuid: "alert-2", status: "dismissed" }],
				})),
			},
		});
		await settle();
		expect(await markerRows("--color-marker")).toHaveLength(0);
		expect(await markerRows("--color-text-muted")).not.toHaveLength(0);
	});

	// Kills a readout that offers the page's own alert again, a change the page
	// cannot make for this alert or this reader, and Dismiss on an alert the API
	// silenced.
	test("offers only what the page and the reader can do", async () => {
		const [alerts, setAlerts] = createSignal<PlotAlerts>({
			status: () => undefined,
		});
		const offer = (next: PlotAlerts) => setAlerts(() => next);
		const root = document.createElement("div");
		root.style.width = "900px";
		document.body.append(root);
		dispose = render(
			() => (
				<Plot
					data={ALERTED}
					hidden={[]}
					onHiddenChange={() => {}}
					onFocusChange={() => {}}
					alerts={alerts()}
				/>
			),
			root,
		);
		await settle();
		const { over, box } = await plotArea();
		await page.elementLocator(over).click({
			position: { x: box.width / 2, y: box.height / 2 },
		});
		const status = page.getByRole("status");
		await expect.element(status).toHaveTextContent("30.00 µs");
		await expect
			.element(status.getByRole("link", { name: /^Alert/ }))
			.not.toBeInTheDocument();
		await expect
			.element(status.getByRole("button", { name: /the alert/ }))
			.not.toBeInTheDocument();

		offer({ href: (uuid) => `/alerts/${uuid}` });
		await expect
			.element(status.getByRole("link", { name: `Alert: ${ALERT_NAME}` }))
			.toBeInTheDocument();
		await expect
			.element(status.getByRole("button", { name: /the alert/ }))
			.not.toBeInTheDocument();

		offer({
			href: (uuid) => `/alerts/${uuid}`,
			change: () => undefined,
		});
		await expect
			.element(status.getByRole("link", { name: /^Alert/ }))
			.toBeInTheDocument();
		await expect
			.element(status.getByRole("button", { name: /the alert/ }))
			.not.toBeInTheDocument();

		offer({
			href: (uuid) => `/alerts/${uuid}`,
			status: () => "silenced",
			change: () => () => {},
		});
		await expect
			.element(status.getByRole("link", { name: /^Alert/ }))
			.toBeInTheDocument();
		await expect
			.element(status.getByRole("button", { name: /the alert/ }))
			.not.toBeInTheDocument();
	});

	// Kills a refusal the readout keeps to itself, and one it shows for another alert.
	test("a pinned readout says when its alert's last change did not go through", async () => {
		const [refused, setRefused] = createSignal<string>();
		mount({
			data: ALERTED,
			alerts: {
				href: (uuid) => `/alerts/${uuid}`,
				change: () => () => {},
				refused: (uuid) => refused() === uuid,
			},
		});
		await settle();
		const { over, box } = await plotArea();
		await page.elementLocator(over).click({
			position: { x: box.width / 2, y: box.height / 2 },
		});
		const status = page.getByRole("status");
		await expect
			.element(status.getByRole("link", { name: `Alert: ${ALERT_NAME}` }))
			.toBeInTheDocument();
		await expect.element(status.getByRole("alert")).not.toBeInTheDocument();
		setRefused("another");
		await expect.element(status.getByRole("alert")).not.toBeInTheDocument();
		setRefused("alert-2");
		await expect.element(status.getByRole("alert")).toBeInTheDocument();
	});

	// Kills a focused line that follows the alerts' statuses, which moves the
	// readout off a line the reader just dismissed and takes its Reactivate.
	test("a Dismiss keeps the alert's line in focus", async () => {
		const [status, setStatus] = createSignal<"active" | "dismissed">();
		mount({
			data: testData([
				testLine({ id: "quiet", benchmark: "crc32c", y: [1, 1, 1, 1, 1] }),
				...ALERTED.lines,
			]),
			alerts: {
				href: (uuid) => `/alerts/${uuid}`,
				status: () => status(),
				change: () => setStatus,
			},
		});
		await settle();
		const { over } = await plotArea();
		over.focus();
		await userEvent.keyboard("{ArrowRight}{ArrowRight}{ArrowRight}");
		await page
			.getByRole("button", { name: `Dismiss the alert on ${ALERT_NAME}` })
			.click();
		await expect
			.element(
				page.getByRole("button", {
					name: `Reactivate the alert on ${ALERT_NAME}`,
				}),
			)
			.toHaveFocus();
	});

	// Kills an alert's page whose plot opens with nothing pinned, or pinned at another point.
	test("pins a point as it first draws", async () => {
		mount({ data: ALERTED, pin: 2 });
		const status = page.getByRole("status");
		await expect.element(status).toHaveTextContent("30.00 µs");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await expect
			.element(page.getByRole("button", { name: "Close the readout" }))
			.toBeInTheDocument();
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
		// One tap on each of the five points, since a tap on the pinned one closes it.
		for (const fraction of [0.05, 0.3, 0.5, 0.7, 0.95]) {
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

	// Kills a pinned readout grown by a line for its close button, which runs a six-line readout past a phone's plot onto its axis labels (decision 77).
	test("keeps a pinned six-line readout inside a phone's plot", async () => {
		await page.viewport(375, 667);
		mount({ data: WIDE }, 343);
		await settle();
		const { over, box } = await plotArea();
		over.dispatchEvent(
			new PointerEvent("click", {
				clientX: box.left + box.width / 2,
				clientY: box.top + box.height / 2,
				bubbles: true,
				pointerType: "touch",
			}),
		);
		await settle();
		expect(readoutBox().bottom).toBeLessThanOrEqual(box.bottom);
	});

	// Kills the readout's whole report hash, which widens an alert page's pinned
	// readout on a phone over the alert's own marker at the plot's newest point.
	test("keeps a phone's pinned readout off the alert's marker", async () => {
		await page.viewport(390, 844);
		const data = testData([
			testLine({
				id: "alerted",
				benchmark: "blake3",
				alerting: true,
				y: [10_000, 10_000, 10_000, 10_000, 30_000],
				upper: [12_000, 12_000, 12_000, 12_000, 12_000],
				alerts: [4],
				raised: [{ index: 4, uuid: "alert-4", status: "active" }],
			}),
		]);
		mount(
			{
				data: {
					...data,
					reports: data.reports.map((report, index) => ({
						...report,
						hash: `${index}`.padStart(40, "a"),
					})),
				},
				pin: 4,
			},
			358,
		);
		await settle();
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("30.00 µs");
		const { box } = await plotArea();
		// The marker's radius on a phone.
		expect(readoutBox().right).toBeLessThanOrEqual(box.right - 6.5);
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
		expect(
			page.getByRole("button", { name: /: hide its line$/ }).all(),
		).toHaveLength(6);
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
			await page
				.getByRole("button", { name: /^sha256.*: hide its line$/ })
				.click();
			await settle();
		} finally {
			CanvasRenderingContext2D.prototype.fillText = fillText;
		}
		expect(drawn.some(({ text }) => text.startsWith("100."))).toBe(true);
		for (const { text, right } of drawn) {
			expect(right, text).toBeLessThanOrEqual(canvas.width);
		}
	});

	// Kills an axis in the tier of its smallest tick, which reads a range of minutes in nanoseconds.
	test("an axis takes the tier of its largest tick", async () => {
		mount({
			data: testData([
				testLine({ id: "zero", benchmark: "crc32c", y: [0, 0, 0] }),
				testLine({ id: "big", benchmark: "sha256", y: [1e12, 1e12, 1e12] }),
			]),
		});
		await settle();
		await expect
			.element(page.getByText("Latency, m", { exact: true }))
			.toBeInTheDocument();
	});

	// Kills a y axis uPlot gives up on past its own steps, which draws no labels.
	test("labels a y axis at 1e300", async () => {
		const drawn: string[] = [];
		const fillText = CanvasRenderingContext2D.prototype.fillText;
		CanvasRenderingContext2D.prototype.fillText = function (
			this: CanvasRenderingContext2D,
			...args: Parameters<typeof fillText>
		) {
			drawn.push(args[0]);
			return fillText.apply(this, args);
		};
		try {
			mount({
				data: testData([testLine({ id: "huge", y: [1e300, 2e300, 3e300] })]),
			});
			await settle();
			await plotArea();
		} finally {
			CanvasRenderingContext2D.prototype.fillText = fillText;
		}
		expect(
			drawn.filter((text) => text.endsWith("e300")).length,
		).toBeGreaterThanOrEqual(3);
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

		await page.getByRole("button", { name: "sha256: hide its line" }).click();
		await settle();

		expect((await plotArea()).box.left).toBe(before);
	});

	// Kills x labels that clip at either edge of the canvas beside a narrow y axis, and minute labels that read the same across seconds.
	test.each([
		["one plot", 1, "dual"],
		["a dual plot", 2, "dual"],
		["stacked plots", 2, "stacked"],
	] as const)(
		"keeps every x label of %s over half a minute inside the canvas, to the second",
		async (_, measures, layout) => {
			const seconds = Array.from({ length: 31 }, (_, index) => index);
			const data = testData(
				Array.from({ length: measures }, (_, measure) =>
					testLine({
						id: `${measure}`,
						measure,
						y: seconds.map((second) => second % 3),
					}),
				),
				[
					{ name: "Latency", units: "nanoseconds (ns)" },
					{ name: "Throughput", units: "operations (ops)" },
				].slice(0, measures),
			);
			const drawn: {
				text: string;
				left: number;
				right: number;
				width: number;
			}[] = [];
			const fillText = CanvasRenderingContext2D.prototype.fillText;
			CanvasRenderingContext2D.prototype.fillText = function (
				this: CanvasRenderingContext2D,
				...args: Parameters<typeof fillText>
			) {
				const [text, x] = args;
				if (this.textAlign === "center") {
					const half = this.measureText(text).width / 2;
					drawn.push({
						text,
						left: x - half,
						right: x + half,
						width: this.canvas.width,
					});
				}
				return fillText.apply(this, args);
			};
			try {
				mount({
					data: {
						...data,
						x: seconds.map((second) => START + (second - 30) * 1000),
					},
					layout,
				});
				await settle();
			} finally {
				CanvasRenderingContext2D.prototype.fillText = fillText;
			}
			expect(drawn.length).toBeGreaterThanOrEqual(2);
			for (const { text, left, right, width } of drawn) {
				expect(text).toMatch(/\d\d:\d\d:\d\d$/);
				expect(left, text).toBeGreaterThanOrEqual(0);
				expect(right, text).toBeLessThanOrEqual(width);
			}
		},
	);
});

// Two measures on a dual axis, the first line guarded and alerting.
const DUAL = testData(
	[
		testLine({
			id: "lat",
			alerting: true,
			y: [20, 20, 40, 20, 20],
			upper: [30, 30, 30, 30, 30],
			alerts: [2],
		}),
		testLine({
			id: "thr",
			benchmark: "sha256",
			measure: 1,
			y: [3000, 3100, 3200, 3100, 3000],
		}),
	],
	[
		{ name: "Latency", units: "nanoseconds (ns)" },
		{ name: "Throughput", units: "operations / second (ops/s)" },
	],
);
// As long as the seed's titles.
const LEFT = "Latency, ns";
const RIGHT = "Throughput, ops/s";
const NOTE = "4w, ending at this report";
const BOUNDARY = /^boundary: /;

const textOf = (text: string | RegExp) =>
	page.getByText(text, { exact: true }).element();
const boxOf = (text: string | RegExp) => textOf(text).getBoundingClientRect();
/** The pill around `text`, which a title and the boundary wear. */
const pillOf = (text: string | RegExp) => {
	const pill = textOf(text).closest(".pl-pill");
	if (!pill) {
		throw new Error(`${text} wears no pill`);
	}
	return pill;
};
const centerOf = (box: DOMRect) => box.top + box.height / 2;
const areaOf = (over: HTMLElement) =>
	(over.closest(".pl-area") as HTMLElement).getBoundingClientRect();

describe("Plot axes row", () => {
	// Kills a head band kept above the axes row, a boundary, note, or alert
	// count off the titles' row, dual titles that swap sides or sit together,
	// and line style glyphs on the wrong titles.
	test("one row above a dual plot holds each axis's title over its axis, with the boundary, note, and alert count between", async () => {
		mount({ data: DUAL, focused: "lat", note: NOTE });
		await settle();
		const { over } = await plotArea("2 lines, Latency and Throughput");
		const area = areaOf(over);
		const figure = page.getByRole("figure").element().getBoundingClientRect();
		const left = pillOf(LEFT).getBoundingClientRect();
		const right = pillOf(RIGHT).getBoundingClientRect();
		const boundary = pillOf(BOUNDARY).getBoundingClientRect();
		const note = boxOf(NOTE);
		const alert = boxOf("1 alert");

		expect(left.top - figure.top).toBeLessThanOrEqual(12);
		for (const item of [right, boundary, note, alert]) {
			expect(centerOf(item)).toBeCloseTo(centerOf(left), 0);
		}
		expect(left.bottom).toBeLessThanOrEqual(area.top);
		expect(left.left).toBeCloseTo(area.left, 0);
		expect(right.right).toBeCloseTo(area.right, 0);
		expect(left.right).toBeLessThan(boundary.left);
		expect(boundary.right).toBeLessThan(note.left);
		expect(note.right).toBeLessThan(alert.left);
		expect(alert.right).toBeLessThan(right.left);
		const stroke = (text: string) =>
			pillOf(text).querySelector("svg")?.getAttribute("data-stroke");
		expect(stroke(LEFT)).toBeNull();
		expect(stroke(RIGHT)).toBe("dotted");
	});

	// Kills stacked panels that repeat the boundary, note, or alert count in
	// every row, or keep them off the first, and a boundary hidden though it
	// has room to read.
	test("stacked panels each head with their own title, and the first row holds the boundary, note, and alert count", async () => {
		mount({ data: DUAL, focused: "lat", note: NOTE, layout: "stacked" });
		await settle();
		await plotArea("1 line, Throughput");
		const first = centerOf(pillOf(LEFT).getBoundingClientRect());
		const second = centerOf(pillOf(RIGHT).getBoundingClientRect());
		expect(second).toBeGreaterThan(first);
		for (const text of [NOTE, "1 alert", BOUNDARY]) {
			expect(page.getByText(text, { exact: true }).elements()).toHaveLength(1);
			expect(centerOf(boxOf(text))).toBeCloseTo(first, 0);
		}
		expect(getComputedStyle(pillOf(BOUNDARY)).opacity).toBe("1");
	});

	// Kills a phone's two-axis titles that share their row with the boundary,
	// note, or alert count, or that are cut, a second row that moves the plot
	// when the boundary appears, and items that overlap.
	test.each([
		[390, 362],
		[320, 292],
	] as const)(
		"on a %d px phone a two-axis plot's titles keep a row of their own, whole, over a row for the rest",
		async (screen, width) => {
			await page.viewport(screen, 844);
			mount({ data: DUAL, focused: null, note: NOTE }, width);
			await settle();
			const { over } = await plotArea("2 lines, Latency and Throughput");
			const area = areaOf(over);
			const left = pillOf(LEFT).getBoundingClientRect();
			const right = pillOf(RIGHT).getBoundingClientRect();
			const alert = boxOf("1 alert");
			for (const text of [LEFT, RIGHT, NOTE]) {
				const element = textOf(text);
				expect(element.scrollWidth, text).toBeLessThanOrEqual(
					element.clientWidth,
				);
			}
			expect(centerOf(right)).toBeCloseTo(centerOf(left), 0);
			expect(left.left).toBeCloseTo(area.left, 0);
			expect(right.right).toBeCloseTo(area.right, 0);

			await page.getByRole("button", { name: "Focus line of blake3" }).click();
			await expect.element(page.getByText(BOUNDARY)).toBeInTheDocument();

			expect(areaOf(over).top).toBe(area.top);
			expect(boxOf("1 alert").left).toBe(alert.left);
			const second = [
				pillOf(BOUNDARY).getBoundingClientRect(),
				boxOf(NOTE),
				boxOf("1 alert"),
			];
			second.forEach((item, index) => {
				expect(item.top, `item ${index}`).toBeGreaterThanOrEqual(left.bottom);
				expect(item.bottom, `item ${index}`).toBeLessThanOrEqual(area.top);
				expect(item.left, `item ${index}`).toBeGreaterThanOrEqual(
					second[index - 1]?.right ?? area.left,
				);
			});
			expect(alert.right).toBeCloseTo(area.right, 0);
		},
	);

	// Kills a boundary cut to a sliver where too little room is left to read it.
	test("a phone row with too little room for the boundary shows none of it", async () => {
		await page.viewport(320, 844);
		const data = testData(
			[
				testLine({
					id: "a",
					alerting: true,
					y: [3000, 3000, 4000, 3000, 3000],
					upper: [3500, 3500, 3500, 3500, 3500],
					alerts: [2],
				}),
			],
			[{ name: "Total throughput", units: "operations / second (ops/s)" }],
		);
		mount({ data, note: NOTE }, 292);
		await settle();
		await plotArea();
		const room = textOf(NOTE).parentElement as HTMLElement;
		expect(room.contains(textOf(BOUNDARY))).toBe(true);
		expect(room.getBoundingClientRect().width).toBeLessThan(64);
		expect(getComputedStyle(pillOf(BOUNDARY)).opacity).toBe("0");
	});

	// Kills a tile that draws the boundary, note, or alert count in its row.
	test("an alerting tile's row holds its titles alone", async () => {
		mount({ data: DUAL, focused: "lat", note: NOTE, size: "tile" }, 540);
		await settle();
		await plotArea("2 lines, Latency and Throughput");
		expect(pillOf(LEFT).textContent).toBe(LEFT);
		for (const text of [NOTE, "1 alert", BOUNDARY]) {
			expect(page.getByText(text, { exact: true }).elements()).toHaveLength(0);
		}
	});

	// Kills a phone's two-axis frame that gains its second row only once the data arrives.
	test("a two-axis plot on a phone holds its frame from the reserve to the drawn plot", async () => {
		await page.viewport(390, 844);
		const { root, setData } = mount(
			{ data: undefined, reserve: { lines: 2, measures: 2 }, note: NOTE },
			362,
		);
		await settle();
		const figure = root.querySelector("figure") as HTMLElement;
		const before = figure.getBoundingClientRect().height;
		setData(DUAL);
		await settle();
		await plotArea("2 lines, Latency and Throughput");
		expect(figure.getBoundingClientRect().height).toBe(before);
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

// Three reports, the middle one run three times at 10, 30, and 20 ns; none
// alerted, so its line runs through its last iteration.
const RAN_THRICE: PlotData = {
	x: [START, START + DAY, START + DAY, START + DAY, START + 2 * DAY],
	report: [0, 1, 1, 1, 2],
	iteration: [0, 0, 1, 2, 0],
	reports: [0, 1, 2].map((index) => ({
		uuid: `report-${index}`,
		version: 100 + index,
		hash: `${index}`.padStart(7, "a"),
	})),
	measures: [{ name: "Latency", units: "nanoseconds (ns)" }],
	lines: [testLine({ id: "ran", y: [20, 10, 30, 20, 20] })],
};

// The middle report's first and third iterations both alerted.
const TWO_ALERTS: PlotData = {
	...RAN_THRICE,
	lines: [
		testLine({
			id: "ran",
			alerting: true,
			y: [20, 30, 12, 31, 20],
			upper: [25, 25, 25, 25, 25],
			alerts: [1, 3],
			raised: [
				{ index: 1, uuid: "first", status: "active" },
				{ index: 3, uuid: "third", status: "active" },
			],
		}),
	],
};

// The middle report's iterations each carry their own upper limit; its line runs through the last.
const BAND: PlotData = {
	...RAN_THRICE,
	lines: [
		testLine({
			id: "ran",
			y: [20, 14, 18, 16, 20],
			upper: [25, 25, 40, 32, 25],
		}),
	],
};

/** Down the middle report's column: its opaque marks' extent, and how many rows between them are drawn see-through. */
const middleColumn = async () => {
	const { canvas, box } = await plotArea();
	const column = box.left + box.width / 2;
	const color = series(1);
	const marks = rowsOf(canvas, column, box.top, box.bottom, color);
	const top = Math.min(...marks);
	const bottom = Math.max(...marks);
	let faint = 0;
	for (let y = top; y <= bottom; y++) {
		const pixel = pixelAt(canvas, column, y);
		if (pixel.alpha > 60 && pixel.alpha < 200 && near(pixel.rgb, color)) {
			faint++;
		}
	}
	return { top, bottom, faint };
};

describe("Plot iterations", () => {
	// Kills a report's iterations drawn as the line's own zigzag, which is
	// opaque, and a bar left out or cut short of either end.
	test("draws a report's iterations as a see-through bar from the lowest to the highest", async () => {
		mount({ data: RAN_THRICE });
		await settle();
		const { top, bottom, faint } = await middleColumn();
		expect(bottom - top).toBeGreaterThan(40);
		expect(faint).toBeGreaterThan((bottom - top) * 0.6);
	});

	// Kills a readout that reads out one iteration, or emphasizes another than the one pointed at.
	test("reads out every iteration of the report, the pointed one emphasized", async () => {
		mount({ data: RAN_THRICE });
		await settle();
		const { over, box } = await plotArea();
		const { top, bottom } = await middleColumn();
		const status = page.getByRole("status");
		const emphasized = () => status.element().querySelector(".pl-rrow.focused");

		await page.elementLocator(over).hover({
			position: { x: box.width / 2, y: bottom - box.top },
		});
		await expect.element(status).toHaveTextContent(/iteration 1\s*10\.00 ns/);
		await expect.element(status).toHaveTextContent(/iteration 2\s*30\.00 ns/);
		await expect.element(status).toHaveTextContent(/iteration 3\s*20\.00 ns/);
		await expect.poll(() => emphasized()?.textContent).toMatch(/^iteration 1/);

		await page.elementLocator(over).hover({
			position: { x: box.width / 2, y: top - box.top },
		});
		await expect.poll(() => emphasized()?.textContent).toMatch(/^iteration 2/);
	});

	// Kills a click on another iteration of the pinned report that unpins
	// instead of pinning it, and a click on the pinned iteration that keeps it.
	test("a click pins the iteration pointed at, another moves the pin, and the same one unpins it", async () => {
		mount({ data: RAN_THRICE });
		await settle();
		const { over, box } = await plotArea();
		const { top, bottom } = await middleColumn();
		const status = page.getByRole("status");
		const emphasized = () => status.element().querySelector(".pl-rrow.focused");
		const at = (y: number) => ({
			position: { x: box.width / 2, y: y - box.top },
		});

		await page.elementLocator(over).click(at(bottom));
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await expect.poll(() => emphasized()?.textContent).toMatch(/^iteration 1/);

		await page.elementLocator(over).click(at(top));
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await expect.poll(() => emphasized()?.textContent).toMatch(/^iteration 2/);

		await page.elementLocator(over).click(at(top));
		await expect.element(status).toHaveAttribute("aria-live", "off");
	});

	// Kills an iteration no report ran drawing an empty plot, and a choice that
	// labels the points of data whose reports each ran once.
	test("draws every iteration when no report ran the chosen one", async () => {
		mount({ data: RAN_THRICE, iteration: 4 });
		await settle();
		const { top, bottom, faint } = await middleColumn();
		expect(bottom - top).toBeGreaterThan(40);
		expect(faint).toBeGreaterThan((bottom - top) * 0.6);

		dispose?.();
		document.body.replaceChildren();
		mount({
			data: testData([testLine({ id: "once", y: [10, 20, 30] })]),
			iteration: 1,
		});
		await settle();
		const { over, box } = await plotArea();
		await page.elementLocator(over).hover({
			position: { x: box.width / 2, y: box.height / 2 },
		});
		const status = page.getByRole("status");
		await expect.element(status).toHaveTextContent("20.00 ns");
		await expect.element(status).not.toHaveTextContent("iteration");
	});

	// Kills a chosen iteration the plot still draws the others of, or does not name.
	test("draws and reads out only the chosen iteration", async () => {
		mount({ data: RAN_THRICE, iteration: 2 });
		await settle();
		const { over, box } = await plotArea();
		const { faint } = await middleColumn();
		expect(faint).toBeLessThan(4);
		await page.elementLocator(over).hover({
			position: { x: box.width / 2, y: box.height / 2 },
		});
		const status = page.getByRole("status");
		await expect.element(status).toHaveTextContent(/iteration 2\s*30\.00 ns/);
		await expect.element(status).not.toHaveTextContent("iteration 1");
	});

	// Kills a pin from the page, as an alert's page sets one, on no line: with
	// two alerts in one report it emphasizes the iteration the line runs
	// through rather than the pinned alert, and a first click on another
	// iteration unpins instead of moving the pin there.
	test("a pin from the page emphasizes its own iteration, and a click on another moves it", async () => {
		mount({
			data: TWO_ALERTS,
			pin: 1,
			alerts: { status: (uuid) => (uuid === "first" ? "active" : undefined) },
		});
		await settle();
		const status = page.getByRole("status");
		const emphasized = () => status.element().querySelector(".pl-rrow.focused");
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await expect.poll(() => emphasized()?.textContent).toMatch(/^iteration 1/);

		const { over, box } = await plotArea();
		const { bottom } = await middleColumn();
		await page.elementLocator(over).click({
			position: { x: box.width / 2, y: bottom - box.top },
		});
		await expect.element(status).toHaveAttribute("aria-live", "polite");
		await expect.poll(() => emphasized()?.textContent).toMatch(/^iteration 2/);
	});

	// Kills a band and limits drawn through every iteration, whose run reaches
	// the middle report at its first iteration's limit rather than the limit of
	// the iteration the line runs through.
	test("draws the band through the iteration the line runs through", async () => {
		mount({ data: BAND });
		await settle();
		const { canvas, box } = await plotArea();
		const color = series(1);
		const bandTop = (column: number) => {
			for (let y = Math.ceil(box.top); y < box.bottom; y++) {
				const pixel = pixelAt(canvas, column, y);
				if (pixel.alpha > 15 && pixel.alpha < 120 && near(pixel.rgb, color)) {
					return y;
				}
			}
			return Number.NaN;
		};
		const start = bandTop(box.left + 3);
		const before = bandTop(box.left + box.width * 0.375);
		expect(before).toBeLessThan(start - 4);
	});

	// Kills range bars drawn for a hidden line, or for a line another's focus dims.
	test("draws no range bars for a hidden or dimmed line", async () => {
		const data: PlotData = {
			...RAN_THRICE,
			lines: [
				...RAN_THRICE.lines,
				testLine({
					id: "other",
					benchmark: "sha256",
					y: [25, null, null, 25, 25],
				}),
			],
		};
		const faintDown = async () => {
			const { canvas, box } = await plotArea();
			const column = box.left + box.width / 2;
			let faint = 0;
			for (let y = Math.ceil(box.top); y < box.bottom; y++) {
				const pixel = pixelAt(canvas, column, y);
				if (
					pixel.alpha > 60 &&
					pixel.alpha < 200 &&
					near(pixel.rgb, series(1))
				) {
					faint++;
				}
			}
			return faint;
		};
		mount({ data, focused: null });
		await settle();
		expect(await faintDown()).toBeGreaterThan(20);

		dispose?.();
		document.body.replaceChildren();
		mount({ data, focused: "other" });
		await settle();
		expect(await faintDown()).toBe(0);

		dispose?.();
		document.body.replaceChildren();
		// Nothing focused, so the hidden line is not dimmed as well.
		mount({ data, hidden: ["ran"], focused: null });
		await settle();
		expect(await faintDown()).toBe(0);
	});

	// Kills marks thinned by a report's iterations: forty reports fit their
	// marks, though their hundred and twenty points would not.
	test("counts reports, not iterations, toward the room its marks need", async () => {
		const reports = 40;
		const data: PlotData = {
			x: Array.from(
				{ length: reports * 3 },
				(_, at) => START + Math.floor(at / 3) * DAY,
			),
			report: Array.from({ length: reports * 3 }, (_, at) =>
				Math.floor(at / 3),
			),
			iteration: Array.from({ length: reports * 3 }, (_, at) => at % 3),
			reports: Array.from({ length: reports }, (_, index) => ({
				uuid: `report-${index}`,
				version: 100 + index,
			})),
			measures: [{ name: "Latency", units: "nanoseconds (ns)" }],
			lines: [
				testLine({
					id: "many",
					y: Array.from(
						{ length: reports * 3 },
						(_, at) => [10, 30, 20][at % 3] ?? 20,
					),
				}),
			],
		};
		mount({ data });
		await settle();
		const { canvas, box } = await plotArea();
		const rows = rowsOf(
			canvas,
			box.left + (box.width * 20) / (reports - 1),
			box.top,
			box.bottom,
			series(1),
		);
		expect(Math.max(...rows) - Math.min(...rows)).toBeGreaterThan(20);
	});

	// Kills a delta read out beside every iteration rather than only the emphasized one.
	test("reads out the delta beside the emphasized iteration alone", async () => {
		mount({
			data: {
				...RAN_THRICE,
				lines: [
					testLine({
						id: "ran",
						y: [20, 10, 30, 22, 20],
						baseline: [20, 20, 20, 20, 20],
						upper: [40, 40, 40, 40, 40],
					}),
				],
			},
		});
		await settle();
		const { over } = await plotArea();
		over.focus();
		await userEvent.keyboard("{ArrowRight}{ArrowRight}");
		const status = page.getByRole("status");
		await expect.element(status).toHaveTextContent(/iteration 3\s*22\.00 ns/);
		const withDelta = [...status.element().querySelectorAll(".pl-rrow")].filter(
			(row) => row.textContent?.includes("%"),
		);
		expect(withDelta.map((row) => row.classList.contains("focused"))).toEqual([
			true,
		]);
	});
});
