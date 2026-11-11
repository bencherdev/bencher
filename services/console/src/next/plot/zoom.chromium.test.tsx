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

const zoom = () => page.getByRole("group", { name: "Zoom" });
const zoomOut = () => page.getByRole("button", { name: "Zoom out" });
const reset = () => page.getByRole("button", { name: "Reset zoom" });
const close = () => page.getByRole("button", { name: "Close the readout" });
/** The band a drag sweeps, the plot area's last child, after uPlot's cursor. */
const band = (over: HTMLElement) => over.lastElementChild as HTMLElement;
const shown = (element: HTMLElement) =>
	getComputedStyle(element).display !== "none";

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
		expect(zoom().query()).toBeNull();
		await close().click();

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

		await expect.element(zoom()).toBeInTheDocument();
		expect(shown(band(over))).toBe(false);
		expect(close().query()).toBeNull();
		await expect.element(status).toHaveAttribute("aria-live", "off");
	});

	// Kills a zoom to a range that holds a single point, and a band left behind.
	test("a drag over 1 point changes nothing and clears its band", async () => {
		mount({ data: RISING });
		await settle();
		const { over, box } = await plotArea();
		await drag(over, at(3.7), at(4.3));
		expect(zoom().query()).toBeNull();
		expect(shown(band(over))).toBe(false);
		expect(close().query()).toBeNull();
		await page
			.elementLocator(over)
			.hover({ position: { x: 1, y: box.height / 2 } });
		await expect
			.element(page.getByRole("status"))
			.toHaveTextContent("10.00 ns");
	});

	// Kills a Zoom out that goes back to the full range or nowhere, a Reset or
	// a double click that leaves the plot zoomed, and a readout that names
	// anything but the span in view.
	test("2 drags, then Zoom out, show the first range, and Reset and a double click show the full range", async () => {
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
		await expect.element(zoom()).toHaveTextContent("Sep 3 to Sep 8");
		expect(first).not.toEqual(full);
		await expect.element(zoomOut()).toHaveAccessibleDescription("Zoom out");
		await expect.element(reset()).toHaveAccessibleDescription("Reset zoom");

		// Reports 3 to 5 of the 1.8 to 7.2 in view.
		const second = xLabels(
			await drawnDuring(() => drag(over, 1 / 5.4, 3.4 / 5.4)),
		);
		await expect.element(zoom()).toHaveTextContent("Sep 4 to Sep 6");
		expect(second).not.toEqual(first);

		expect(xLabels(await drawnDuring(() => zoomOut().click()))).toEqual(first);
		await expect.element(zoom()).toHaveTextContent("Sep 3 to Sep 8");

		expect(xLabels(await drawnDuring(() => reset().click()))).toEqual(full);
		expect(zoom().query()).toBeNull();
		// The control that held the focus is gone, so the plot area takes it.
		expect(document.activeElement).toBe(over);

		await drag(over, at(1.8), at(7.2));
		await expect.element(zoom()).toBeInTheDocument();
		const box = over.getBoundingClientRect();
		expect(
			xLabels(
				await drawnDuring(() =>
					page.elementLocator(over).dblClick(spot(box, 0.5)),
				),
			),
		).toEqual(full);
		expect(zoom().query()).toBeNull();
	});

	// Kills a readout on the version axis that names dates or the drag's own ends.
	test("names the versions in view on the version axis", async () => {
		mount({ data: RISING, xAxis: "version" });
		await settle();
		const { over } = await plotArea();
		await drag(over, at(2.8), at(5.2));
		await expect.element(zoom()).toHaveTextContent("103 to 105");
	});

	// Kills a zoom that reaches only the panel dragged, as the sync key would
	// carry it, a y axis that keeps the full range, and a Zoom out that leaves
	// a panel behind.
	test("a drag on 1 panel of a stack zooms every panel, and Zoom out moves every panel back", async () => {
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

		const drawn = await drawnDuring(() => drag(top.over, at(2.8), at(5.2)));
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

		const back = await drawnDuring(() => zoomOut().click());
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
		await expect.element(zoom()).toBeInTheDocument();
		expect(close().query()).toBeNull();
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
		await expect.element(zoom()).toHaveTextContent("Sep 4 to Sep 6");

		await page.getByRole("button", { name: "sha256: hide its line" }).click();
		await settle();
		await expect.element(zoom()).toHaveTextContent("Sep 4 to Sep 6");

		setData(
			testData([
				testLine({ id: "a", benchmark: "crc32c", y: [1, 2, 3, 4, 5] }),
			]),
		);
		await settle();
		expect(zoom().query()).toBeNull();
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
		expect(zoom().query()).toBeNull();
		expect(close().query()).toBeNull();
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
		expect(zoom().query()).toBeNull();
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
		expect(zoom().query()).toBeNull();
	});

	// Kills a row's zoom footer that holds its room while not zoomed, or never shows.
	test("a row's footer shows only while zoomed", async () => {
		mount({ data: RISING, size: "row" });
		await settle();
		const { over } = await plotArea();
		const figure = page.getByRole("figure").element();
		const height = figure.getBoundingClientRect().height;
		expect(zoom().query()).toBeNull();

		await drag(over, at(2.8), at(5.2));
		await expect.element(zoom()).toHaveTextContent("Sep 4 to Sep 6");
		expect(figure.contains(zoom().element())).toBe(true);
		expect(figure.getBoundingClientRect().height).toBeGreaterThan(height);

		await reset().click();
		expect(zoom().query()).toBeNull();
		expect(figure.getBoundingClientRect().height).toBe(height);
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
		expect(zoom().query()).toBeNull();
	});

	// Kills a zoom hidden while other data is drawn, which comes back with the data it was made on.
	test("data that comes back after other data does not bring back its zoom", async () => {
		const { setData } = mount({ data: RISING });
		await settle();
		const { over } = await plotArea();
		await drag(over, at(2.8), at(5.2));
		await expect.element(zoom()).toBeInTheDocument();

		setData(testData([testLine({ id: "rising", y: [1, 2, 3, 4, 5] })]));
		await settle();
		expect(zoom().query()).toBeNull();
		setData(RISING);
		await settle();
		await plotArea();
		expect(zoom().query()).toBeNull();
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
		await expect.element(zoom()).toBeInTheDocument();
		expect(close().query()).toBeNull();
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
			expect(zoom().query()).toBeNull();
			expect(close().query()).toBeNull();
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
		await expect.element(zoom()).toHaveTextContent("Sep 4 to Sep 6");
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

	// Kills a note, such as the window, that stays while a zoom names its own span, or never comes back.
	test("the note hides while zoomed and comes back on Reset", async () => {
		const note = "4w, ending at its report";
		mount({ data: RISING, size: "row", note });
		await settle();
		const { over } = await plotArea();
		await expect.element(page.getByText(note)).toBeVisible();

		await drag(over, at(2.8), at(5.2));
		await expect.element(zoom()).toBeInTheDocument();
		expect(page.getByText(note).query()).toBeNull();

		await reset().click();
		await expect.element(page.getByText(note)).toBeVisible();
	});
});
