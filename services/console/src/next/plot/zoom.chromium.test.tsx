import "@bencherdev/ui/styles.css";
import "./plot.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, describe, expect, test } from "vitest";
import { commands, page, userEvent } from "vitest/browser";
import Plot, { type PlotProps } from "./Plot";
import { canvasOf, drawnDuring, settle, xLabels, yValues } from "./pixels";
import { testData, testLine } from "./testing";
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
});

type Harness = Omit<
	PlotProps,
	"data" | "hidden" | "focused" | "onHiddenChange" | "onFocusChange"
> & { data: PlotData };

const mount = (props: Harness) => {
	const root = document.createElement("div");
	root.style.width = "900px";
	document.body.append(root);
	const [data, setData] = createSignal(props.data);
	const [hidden, setHidden] = createSignal<string[]>([]);
	const [focused, setFocused] = createSignal<string | null>();
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
	return { setData };
};

/** The plot area named `name` and its box on the page. */
const plotArea = async (name: RegExp | string = /lines?,/) => {
	const area = page.getByRole("group", { name });
	await expect.element(area).toBeInTheDocument();
	const over = area.element() as HTMLElement;
	return { over, box: over.getBoundingClientRect() };
};

// Ten reports a day apart, so report `index` sits at `index / 9` of the plot area's width.
const at = (index: number) => index / 9;
const RISING = testData([
	testLine({ id: "rising", y: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100] }),
]);
const FALLING = [95, 90, 85, 80, 75, 70, 65, 60, 55, 50];
// Rising under a bound, its last report alerting, so its first axes row holds every chip.
const BOUNDED = testData([
	testLine({
		id: "rising",
		y: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100],
		upper: Array(10).fill(105),
		alerting: true,
		alerts: [9],
	}),
]);
// The same on the left of a dual axis, with a second measure on the right.
const DUAL = testData(
	[
		BOUNDED.lines[0] as PlotData["lines"][number],
		testLine({ id: "falling", measure: 1, y: FALLING }),
	],
	[
		{ name: "Latency", units: "nanoseconds (ns)" },
		{ name: "Wait", units: "nanoseconds (ns)" },
	],
);
/** A box's edges, which a DOMRect, having no own properties, does not compare by. */
const edges = ({ left, top, right, bottom }: DOMRect) => ({
	left,
	top,
	right,
	bottom,
});

/** Near the plot area's foot, under where a readout pins. */
const spot = (box: DOMRect, fraction: number) => ({
	position: { x: box.width * fraction, y: box.height - 12 },
});

/** A mouse's press at `from` and release at `to`, as fractions of the plot area's width. */
const drag = async (over: HTMLElement, from: number, to: number) => {
	const box = over.getBoundingClientRect();
	await page.elementLocator(over).hover(spot(box, from));
	await commands.mouseDown();
	await page.elementLocator(over).hover(spot(box, to));
	await commands.mouseUp();
	await settle();
};

const close = () => page.getByRole("button", { name: "Close the readout" });
/** The band a drag sweeps, the plot area's last child, after uPlot's cursor. */
const band = (over: HTMLElement) => over.lastElementChild as HTMLElement;
const shown = (element: HTMLElement) =>
	getComputedStyle(element).display !== "none";
/** Asserts the first and last reports in view, as Home and End on the plot area read them out, then closes the readout. */
const expectInView = async (over: HTMLElement, first: string, last: string) => {
	const status = page.getByRole("status").first();
	over.focus();
	await userEvent.keyboard("{Home}");
	await expect.element(status).toHaveTextContent(first);
	await userEvent.keyboard("{End}");
	await expect.element(status).toHaveTextContent(last);
	await userEvent.keyboard("{Escape}");
};
const FULL = ["10.00 ns", "100.00 ns"] as const;
/** A double click near the plot area's foot. */
const doubleClick = (over: HTMLElement) =>
	page.elementLocator(over).dblClick(spot(over.getBoundingClientRect(), 0.5));

describe("Plot zoom", () => {
	// Kills a press that zooms or that a small wobble keeps from pinning, a drag
	// that pins at its release or keeps the readout up, and a band drawn
	// anywhere but from the press to the pointer.
	test("a press that moves under 5 px pins, and a drag past 5 px zooms, pins nothing, and drops its click", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		const status = page.getByRole("status");

		await page.elementLocator(over).hover(spot(box, at(3)));
		await commands.mouseDown();
		await page.elementLocator(over).hover({
			position: { x: box.width * at(3) + 3, y: box.height - 12 },
		});
		await commands.mouseUp();
		await expect.element(close()).toBeInTheDocument();
		await close().click();
		await expectInView(over, ...FULL);

		await page.elementLocator(over).hover(spot(box, at(2.8)));
		await expect.element(status).toHaveTextContent("ns");
		await commands.mouseDown();
		await page.elementLocator(over).hover(spot(box, at(5.2)));
		await expect.poll(() => shown(band(over))).toBe(true);
		const drawn = band(over).getBoundingClientRect();
		expect(drawn.left).toBeCloseTo(box.left + box.width * at(2.8), -0.5);
		expect(drawn.right).toBeCloseTo(box.left + box.width * at(5.2), -0.5);
		expect(drawn.top).toBeCloseTo(box.top, 0);
		expect(drawn.bottom).toBeCloseTo(box.bottom, 0);
		await expect.element(status).not.toHaveTextContent("ns");
		await commands.mouseUp();
		await settle();

		expect(shown(band(over))).toBe(false);
		expect(close().query()).toBeNull();
		await expect.element(status).toHaveAttribute("aria-live", "off");
		await expectInView(over, "40.00 ns", "60.00 ns");
	});

	// Kills a zoom to a range that holds a single point, and a band left behind.
	test("a drag over 1 point changes nothing and clears its band", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		await drag(over, at(3.7), at(4.3));
		expect(shown(band(over))).toBe(false);
		expect(close().query()).toBeNull();
		await page
			.elementLocator(over)
			.hover({ position: { x: 1, y: box.height / 2 } });
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("10.00 ns");
	});

	// Kills a double click that goes back to the full range or nowhere, or
	// leaves a readout pinned, and one at the full range that goes anywhere.
	test("2 drags, then a double click, show the first range, and a second double click the full range", async () => {
		let over!: HTMLElement;
		const full = xLabels(
			await drawnDuring(async () => {
				mount({ data: RISING });
				await settle();
				over = (await plotArea()).over;
			}),
		);
		const first = xLabels(
			await drawnDuring(() => drag(over, at(1.8), at(7.2))),
		);
		expect(first).not.toEqual(full);

		// Reports 3 to 5 of the 1.8 to 7.2 in view.
		const second = xLabels(
			await drawnDuring(() => drag(over, 1 / 5.4, 3.4 / 5.4)),
		);
		expect(second).not.toEqual(first);
		await expectInView(over, "40.00 ns", "60.00 ns");

		expect(xLabels(await drawnDuring(() => doubleClick(over)))).toEqual(first);
		expect(close().query()).toBeNull();
		await expectInView(over, "30.00 ns", "80.00 ns");

		expect(xLabels(await drawnDuring(() => doubleClick(over)))).toEqual(full);
		expect(close().query()).toBeNull();
		await expectInView(over, ...FULL);

		await doubleClick(over);
		expect(close().query()).toBeNull();
		await expectInView(over, ...FULL);
	});

	// Kills a run of 3 clicks that steps back twice, as every click after the
	// first would, or that leaves a readout pinned.
	test("a triple click steps back 1 step and pins nothing", async () => {
		mount({ data: RISING });
		await settle();
		const { over } = await plotArea();
		await drag(over, at(1.8), at(7.2));
		await drag(over, 1 / 5.4, 3.4 / 5.4);
		await page
			.elementLocator(over)
			.tripleClick(spot(over.getBoundingClientRect(), 0.5));
		await settle();
		expect(close().query()).toBeNull();
		await expectInView(over, "30.00 ns", "80.00 ns");
	});

	// Kills a drag on the version axis that zooms to anything but the versions it covers.
	test("a drag on the version axis zooms to the versions it covers", async () => {
		mount({ data: RISING, xAxis: "version" });
		await settle();
		const { over } = await plotArea();
		await drag(over, at(2.8), at(5.2));
		await expectInView(over, "40.00 ns", "60.00 ns");
		await doubleClick(over);
		await expectInView(over, ...FULL);
	});

	// Kills a zoom that reaches only the panel dragged, as the sync key would
	// carry it, a y axis that keeps the full range, and a double click that
	// leaves a panel behind or goes back further than 1 step.
	test("2 drags on 1 panel of a stack zoom every panel, and each double click moves every panel back 1 step", async () => {
		const data = testData(
			[
				testLine({
					id: "rising",
					y: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100],
				}),
				testLine({ id: "falling", measure: 1, y: FALLING }),
			],
			[
				{ name: "Latency", units: "nanoseconds (ns)" },
				{ name: "Wait", units: "nanoseconds (ns)" },
			],
		);
		mount({ data, layout: "stacked" });
		await settle();
		const top = await plotArea("1 line, Latency");
		const bottom = await plotArea("1 line, Wait");
		const status = page.getByRole("status");
		const leftEdge = { position: { x: 1, y: bottom.box.height / 2 } };

		await drag(top.over, at(1.8), at(7.2));
		// Reports 3 to 5 of the 1.8 to 7.2 in view.
		const drawn = await drawnDuring(() => drag(top.over, 1 / 5.4, 3.4 / 5.4));
		const latency = yValues(drawn, "left", canvasOf(top.over));
		const wait = yValues(drawn, "left", canvasOf(bottom.over));
		expect(latency.length).toBeGreaterThanOrEqual(2);
		expect(wait.length).toBeGreaterThanOrEqual(2);
		expect(Math.min(...latency)).toBeGreaterThanOrEqual(30);
		expect(Math.max(...latency)).toBeLessThanOrEqual(70);
		expect(Math.min(...wait)).toBeGreaterThanOrEqual(65);
		expect(Math.max(...wait)).toBeLessThanOrEqual(85);
		// Report 3 is the first in view.
		await page.elementLocator(bottom.over).hover(leftEdge);
		await expect.element(status).toHaveTextContent("80.00 ns");

		await doubleClick(bottom.over);
		await expectInView(top.over, "30.00 ns", "80.00 ns");
		await expectInView(bottom.over, "85.00 ns", "60.00 ns");

		const back = await drawnDuring(() => doubleClick(top.over));
		expect(
			Math.max(...yValues(back, "left", canvasOf(top.over))),
		).toBeGreaterThanOrEqual(90);
		expect(
			Math.max(...yValues(back, "left", canvasOf(bottom.over))),
		).toBeGreaterThanOrEqual(90);
		await page.elementLocator(bottom.over).hover(leftEdge);
		await expect.element(status).toHaveTextContent("95.00 ns");
	});

	// Kills a y axis that keeps fitting every point once the x axis zooms, on either side of a dual axis.
	test("after a zoom, each y axis fits the points in view", async () => {
		const data = testData(
			[
				testLine({
					id: "rising",
					y: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100],
				}),
				testLine({ id: "falling", measure: 1, y: FALLING }),
			],
			[
				{ name: "Latency", units: "nanoseconds (ns)" },
				{ name: "Wait", units: "nanoseconds (ns)" },
			],
		);
		let over!: HTMLElement;
		const before = await drawnDuring(async () => {
			mount({ data });
			await settle();
			over = (await plotArea()).over;
		});
		expect(Math.max(...yValues(before, "left"))).toBeGreaterThanOrEqual(90);
		expect(Math.max(...yValues(before, "right"))).toBeGreaterThanOrEqual(90);

		const after = await drawnDuring(() => drag(over, at(2.8), at(5.2)));
		const left = yValues(after, "left");
		const right = yValues(after, "right");
		expect(left.length).toBeGreaterThanOrEqual(2);
		expect(right.length).toBeGreaterThanOrEqual(2);
		expect(Math.min(...left)).toBeGreaterThanOrEqual(30);
		expect(Math.max(...left)).toBeLessThanOrEqual(70);
		expect(Math.min(...right)).toBeGreaterThanOrEqual(65);
		expect(Math.max(...right)).toBeLessThanOrEqual(85);
	});

	// Kills a pinned readout that stays up, at a point the zoom moved, once a drag zooms.
	test("a drag while pinned closes the pin and zooms", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		await page.elementLocator(over).click({
			position: { x: box.width * at(8), y: box.height / 2 },
		});
		await expect.element(close()).toBeInTheDocument();
		await drag(over, at(1.8), at(4.2));
		expect(close().query()).toBeNull();
		await expectInView(over, "30.00 ns", "50.00 ns");
	});

	// Kills a zoom kept over data it was not made on, and one a key toggle drops.
	test("new data clears the zoom, and hiding a line keeps it", async () => {
		const data = testData([
			testLine({
				id: "a",
				benchmark: "crc32c",
				y: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100],
			}),
			testLine({ id: "b", benchmark: "sha256", y: FALLING }),
		]);
		const { setData } = mount({ data });
		await settle();
		const { over } = await plotArea();
		await drag(over, at(2.8), at(5.2));

		await page.getByRole("button", { name: "sha256: hide its line" }).click();
		await settle();
		await expectInView(over, "40.00 ns", "60.00 ns");

		setData(
			testData([
				testLine({ id: "a", benchmark: "crc32c", y: [1, 2, 3, 4, 5] }),
			]),
		);
		await settle();
		await expectInView((await plotArea()).over, "1.00 ns", "5.00 ns");
	});

	// Kills an Escape that lets the drag go on to zoom, or leaves its band or a pin behind.
	test("Escape during a drag cancels it", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		await page.elementLocator(over).hover(spot(box, at(2)));
		await commands.mouseDown();
		await page.elementLocator(over).hover(spot(box, at(6)));
		await expect.poll(() => shown(band(over))).toBe(true);
		await userEvent.keyboard("{Escape}");
		expect(shown(band(over))).toBe(false);
		await page.elementLocator(over).hover(spot(box, at(7)));
		await commands.mouseUp();
		await settle();
		expect(shown(band(over))).toBe(false);
		expect(close().query()).toBeNull();
		await expectInView(over, ...FULL);
	});

	// Kills a finger that drags a zoom, which a phone's swipe would start.
	test("a touch never starts a drag", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		const touch = (type: string, fraction: number) =>
			over.dispatchEvent(
				new PointerEvent(type, {
					clientX: box.left + box.width * fraction,
					clientY: box.top + box.height - 12,
					bubbles: true,
					pointerType: "touch",
					isPrimary: true,
					// A finger on the screen holds the main button down until it lifts.
					buttons: type === "pointerup" ? 0 : 1,
				}),
			);
		touch("pointerdown", at(2));
		touch("pointermove", at(4));
		touch("pointermove", at(6));
		touch("pointerup", at(6));
		await settle();
		expect(shown(band(over))).toBe(false);
		await expectInView(over, ...FULL);
	});

	// Kills a press that inherits the drag of one before it whose release the
	// plot never heard, as when a context menu took it, so the new press drags
	// a stale band and its click pins nothing.
	test("a second press with no move between starts fresh", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		// The mouse's own pointer, which the browser keeps active, so the plot can capture it.
		const mouse = (type: string, fraction: number, buttons: number) => {
			const init = {
				clientX: box.left + box.width * fraction,
				clientY: box.top + box.height - 12,
				bubbles: true,
				pointerId: 1,
				pointerType: "mouse",
				isPrimary: true,
				button: 0,
				buttons,
			};
			over.dispatchEvent(
				type === "click"
					? new MouseEvent(type, init)
					: new PointerEvent(type, init),
			);
		};
		mouse("pointerdown", at(2), 1);
		mouse("pointermove", at(5), 1);
		expect(shown(band(over))).toBe(true);

		mouse("pointerdown", at(7), 1);
		expect(shown(band(over))).toBe(false);
		mouse("pointerup", at(7), 0);
		mouse("click", at(7), 0);
		await expect.element(close()).toBeInTheDocument();
		await close().click();
		await expectInView(over, ...FULL);
	});

	// Kills a y axis that prints the points in view in the tier of a spike out of view, as "0" over "0".
	test("after a zoom past a spike, the y axis labels the points in view", async () => {
		const data = testData(
			[
				testLine({
					id: "spike",
					y: [6.8, 6.82, 6.84, 6.86, 6.88, 6.9, 6.92, 6.94, 6.96, 3.35e9],
				}),
			],
			[{ name: "Screams", units: "dB" }],
		);
		let over!: HTMLElement;
		const before = await drawnDuring(async () => {
			mount({ data, size: "row" });
			await settle();
			over = (await plotArea()).over;
		});
		expect(Math.max(...yValues(before, "left"))).toBeGreaterThan(1e8);

		const after = await drawnDuring(() => drag(over, at(2.8), at(5.2)));
		const left = yValues(after, "left");
		expect(left.length).toBeGreaterThanOrEqual(2);
		expect(new Set(left).size).toBe(left.length);
		expect(Math.min(...left)).toBeGreaterThan(6);
		expect(Math.max(...left)).toBeLessThan(8);

		const back = await drawnDuring(() => doubleClick(over));
		expect(Math.max(...yValues(back, "left"))).toBeGreaterThan(1e8);
	});

	// Kills a press whose release off the plot area is never heard, so its band
	// follows a mouse with no button down and the next click pins nothing.
	test("a press released off the plot leaves no drag behind", async () => {
		mount({ data: RISING });
		const elsewhere = document.createElement("div");
		elsewhere.style.cssText =
			"position: fixed; right: 0; top: 0; width: 200px; height: 200px";
		document.body.append(elsewhere);
		await settle();
		const { over, box } = await plotArea();
		await page.elementLocator(over).hover(spot(box, at(2)));
		await commands.mouseDown();
		await page
			.elementLocator(elsewhere)
			.hover({ position: { x: 100, y: 100 } });
		await commands.mouseUp();
		await page.elementLocator(over).hover(spot(box, at(6)));
		await settle();
		expect(shown(band(over))).toBe(false);

		await page.elementLocator(over).click(spot(box, at(6)));
		await expect.element(close()).toBeInTheDocument();
		await close().click();
		await expectInView(over, ...FULL);
	});

	// Kills a zoom hidden while other data is drawn, which comes back with the data it was made on.
	test("data that comes back after other data does not bring back its zoom", async () => {
		const { setData } = mount({ data: RISING });
		await settle();
		const { over } = await plotArea();
		await drag(over, at(2.8), at(5.2));
		await expectInView(over, "40.00 ns", "60.00 ns");

		setData(testData([testLine({ id: "rising", y: [1, 2, 3, 4, 5] })]));
		await settle();
		await expectInView((await plotArea()).over, "1.00 ns", "5.00 ns");
		setData(RISING);
		await settle();
		await expectInView((await plotArea()).over, ...FULL);
	});

	// Kills a drag that must move well past 5 px before it zooms.
	test("a drag of about 10 px zooms", async () => {
		const many = testData([
			testLine({
				id: "rising",
				y: Array.from({ length: 100 }, (_, index) => index + 1),
			}),
		]);
		mount({ data: many });
		await settle();
		const { over, box } = await plotArea();
		const of = (index: number) => (box.width * index) / 99;
		const y = box.height - 12;
		await page.elementLocator(over).hover({ position: { x: of(40) - 1.5, y } });
		await commands.mouseDown();
		await page.elementLocator(over).hover({ position: { x: of(41) + 1.5, y } });
		await commands.mouseUp();
		await settle();
		expect(of(41) - of(40) + 3).toBeLessThan(15);
		expect(close().query()).toBeNull();
		await expectInView(over, "41.00 ns", "42.00 ns");
	});

	// Kills a drag that goes on once the browser cancels the pointer or takes its capture away.
	test("a cancelled pointer or a lost capture ends the drag", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		let pointer = 0;
		over.addEventListener("pointerdown", (event) => {
			pointer = event.pointerId;
		});
		for (const end of [
			() =>
				over.dispatchEvent(
					new PointerEvent("pointercancel", {
						pointerId: pointer,
						bubbles: true,
					}),
				),
			() => over.releasePointerCapture(pointer),
		]) {
			await page.elementLocator(over).hover(spot(box, at(2)));
			await commands.mouseDown();
			await page.elementLocator(over).hover(spot(box, at(5)));
			await expect.poll(() => shown(band(over))).toBe(true);
			end();
			await page.elementLocator(over).hover(spot(box, at(6)));
			await settle();
			expect(shown(band(over))).toBe(false);
			await commands.mouseUp();
			await settle();
			expect(close().query()).toBeNull();
			await expectInView(over, ...FULL);
		}
	});

	// Kills an Escape that cancels a drag and also reaches the page, where it would close a dialog the plot sits in.
	test("Escape during a drag goes no further than the drag", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		const heard: string[] = [];
		document.body.addEventListener("keydown", (event) => heard.push(event.key));

		await page.elementLocator(over).hover(spot(box, at(2)));
		await commands.mouseDown();
		await page.elementLocator(over).hover(spot(box, at(6)));
		await expect.poll(() => shown(band(over))).toBe(true);
		await userEvent.keyboard("{Escape}");
		await commands.mouseUp();
		expect(heard).toEqual([]);

		over.focus();
		await userEvent.keyboard("{Escape}");
		expect(heard).toEqual(["Escape"]);
	});

	// Kills keys that step to points the zoom left out, which pins a readout off the plot area.
	test("the keys step through the points in view while zoomed", async () => {
		mount({ data: RISING });
		await settle();
		const { over } = await plotArea();
		await drag(over, at(2.8), at(5.2));
		const status = page.getByRole("status");
		over.focus();

		await userEvent.keyboard("{ArrowRight}");
		await expect.element(status).toHaveTextContent("40.00 ns");
		await userEvent.keyboard("{ArrowLeft}");
		await expect.element(status).toHaveTextContent("40.00 ns");
		await userEvent.keyboard("{End}");
		await expect.element(status).toHaveTextContent("60.00 ns");
		await userEvent.keyboard("{ArrowRight}");
		await expect.element(status).toHaveTextContent("60.00 ns");
		await userEvent.keyboard("{Home}");
		await expect.element(status).toHaveTextContent("40.00 ns");
		await userEvent.keyboard("{Escape}");
		await userEvent.keyboard("{ArrowLeft}");
		await expect.element(status).toHaveTextContent("60.00 ns");
	});

	// Kills a zoom that moves a label, adds a control or a span, or hides the
	// note, on a row and on a desktop two-axis plot.
	test.each([
		["a row", BOUNDED, "row"],
		["a desktop two-axis plot", DUAL, "full"],
	] as const)(
		"zooming %s moves no label, keeps its note, and adds no control or span",
		async (_, data, size) => {
			const note = "Cut to 3 months";
			mount({ data, size, note });
			await settle();
			const { over } = await plotArea();
			const figure = page.getByRole("figure").element();
			const chips = () =>
				[
					...figure.querySelectorAll(
						".pl-axes > .pl-pill, .pl-axes > .pl-note, .pl-axes > .pl-axt",
					),
				].map((chip) => edges(chip.getBoundingClientRect()));
			const before = chips();
			expect(before.length).toBeGreaterThanOrEqual(data === DUAL ? 5 : 4);
			const buttons = figure.querySelectorAll("button").length;
			const text = figure.textContent;

			await drag(over, at(2.8), at(5.2));
			await expectInView(over, "40.00 ns", "60.00 ns");
			expect(chips()).toEqual(before);
			await expect.element(page.getByText(note)).toBeVisible();
			expect(figure.querySelectorAll("button")).toHaveLength(buttons);
			expect(figure.textContent).toBe(text);
			expect(page.getByRole("button", { name: /zoom/i }).query()).toBeNull();
		},
	);
});
