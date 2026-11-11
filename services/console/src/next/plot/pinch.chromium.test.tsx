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

// Real fingers, through the browser's own touch input, so a pinch and a swipe
// meet the plot area's touch-action as a phone's would.

let dispose: (() => void) | undefined;

afterEach(async () => {
	await commands.touch("touchEnd", []).catch(() => {});
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
	document.documentElement.removeAttribute(THEME_ATTRIBUTE);
	document.scrollingElement?.scrollTo(0, 0);
});

beforeEach(async () => {
	await page.viewport(414, 896);
	document.documentElement.setAttribute(THEME_ATTRIBUTE, "dark");
});

type Harness = Omit<
	PlotProps,
	"data" | "hidden" | "focused" | "onHiddenChange" | "onFocusChange"
> & { data: PlotData };

const mount = (props: Harness) => {
	const root = document.createElement("div");
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

const plotArea = async (name: RegExp | string = /lines?,/) => {
	const area = page.getByRole("group", { name });
	await expect.element(area).toBeInTheDocument();
	return area.element() as HTMLElement;
};

// Ten reports a day apart, so report `index` sits at `index / 9` of the plot area's width.
const RISING = testData([
	testLine({ id: "rising", y: [10, 20, 30, 40, 50, 60, 70, 80, 90, 100] }),
]);
const FALLING = [95, 90, 85, 80, 75, 70, 65, 60, 55, 50];

type Pair = readonly [number, number];

/** A finger at `fraction` of the plot area's width, `below` CSS pixels under halfway down it. */
const finger = (over: HTMLElement, id: number, fraction: number, below = 0) => {
	const box = over.getBoundingClientRect();
	return {
		id,
		x: box.left + box.width * fraction,
		y: box.top + box.height / 2 + below,
	};
};

/** Two fingers put down at `from`, moved in steps to `to`, as fractions of the plot area's width. */
const spread = async (over: HTMLElement, from: Pair, to: Pair, steps = 4) => {
	await commands.touch("touchStart", [
		finger(over, 1, from[0]),
		finger(over, 2, from[1]),
	]);
	for (let step = 1; step <= steps; step++) {
		const part = step / steps;
		await commands.touch("touchMove", [
			finger(over, 1, from[0] + (to[0] - from[0]) * part),
			finger(over, 2, from[1] + (to[1] - from[1]) * part),
		]);
	}
};

/** A whole pinch: both fingers lift at its end. */
const pinch = async (over: HTMLElement, from: Pair, to: Pair, steps = 4) => {
	await spread(over, from, to, steps);
	await commands.touch("touchEnd", []);
	await settle();
};

const tap = async (over: HTMLElement, fraction: number) => {
	await commands.touch("touchStart", [finger(over, 7, fraction)]);
	await commands.touch("touchEnd", []);
};

/** The page's scroll held still for 5 frames in a row. */
const scrolled = () => {
	let last = Number.NaN;
	let still = 0;
	return expect
		.poll(
			async () => {
				await settle();
				const now = document.scrollingElement?.scrollTop ?? 0;
				still = now === last ? still + 1 : 0;
				last = now;
				return still >= 5;
			},
			{ timeout: 5000 },
		)
		.toBe(true);
};

const zoom = () => page.getByRole("group", { name: "Zoom" });
const close = () => page.getByRole("button", { name: "Close the readout" });

// From 0.15 and 0.35 of the width to 0.05 and 0.45: half the range, around the
// report at 2.25, which keeps its place a quarter of the way across.
const IN: [Pair, Pair] = [
	[0.15, 0.35],
	[0.05, 0.45],
];
const IN_SPAN = "Sep 2 to Sep 7";

describe("Plot pinch", () => {
	// Kills a pinch that zooms only once it ends, scales around the plot's
	// middle rather than its fingers', or pushes a step per move, so 1 Zoom
	// out leaves part of it.
	test("a pinch zooms live around its midpoint, and 1 Zoom out undoes it", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		// In 1 move, so what the fingers draw before they lift is the range they end at.
		const live = xLabels(await drawnDuring(() => spread(over, ...IN, 1)));
		const days = live.map((label) => Number(/^Sep (\d+)/.exec(label)?.[1]));
		expect(days.length).toBeGreaterThan(0);
		for (const day of days) {
			expect(day).toBeGreaterThanOrEqual(2);
			expect(day).toBeLessThanOrEqual(7);
		}
		expect(zoom().query()).toBeNull();
		await commands.touch("touchEnd", []);
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);

		await page.getByRole("button", { name: "Zoom out" }).click();
		expect(zoom().query()).toBeNull();
	});

	// Kills a pinch out that zooms out past the step before, pushes a step,
	// or leaves the full range for a wider one.
	test("a pinch out past where it started pops 1 step, and an empty stack stays at the full range", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		const full = xLabels(
			await drawnDuring(() => pinch(over, [0.3, 0.7], [0.4, 0.6])),
		);
		expect(full.length).toBeGreaterThan(0);
		expect(zoom().query()).toBeNull();

		await pinch(over, ...IN);
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);
		// From 0.3 and 0.7 to a sixth and five sixths: 0.6 of that range, around its middle.
		await pinch(over, [0.3, 0.7], [1 / 6, 5 / 6]);
		await expect.element(zoom()).toHaveTextContent("Sep 3 to Sep 6");

		await pinch(over, [0.2, 0.8], [0.3, 0.7]);
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);
		await pinch(over, [0.2, 0.8], [0.3, 0.7]);
		expect(zoom().query()).toBeNull();
		expect(
			xLabels(await drawnDuring(() => pinch(over, [0.2, 0.8], [0.3, 0.7]))),
		).toEqual(full);
		expect(zoom().query()).toBeNull();
	});

	// Kills a pinch that zooms to a range holding a single point, or none.
	test("a pinch to fewer than 2 points stops at the narrowest range that holds 2", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		await pinch(over, [0.45, 0.55], [0.02, 0.98]);
		await expect.element(zoom()).toBeInTheDocument();

		const status = page.getByRole("status");
		over.focus();
		await userEvent.keyboard("{Home}");
		await expect.element(status).toHaveTextContent("50.00 ns");
		await userEvent.keyboard("{End}");
		await expect.element(status).toHaveTextContent("60.00 ns");
	});

	// Kills a pinch that reaches only the panel under the fingers.
	test("a pinch on 1 panel of a stack zooms every panel", async () => {
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

		// Reports 2 to 5 are in view, drawn in 1 move so only they are.
		const drawn = await drawnDuring(() => pinch(top, ...IN, 1));
		const wait = yValues(drawn, "left", canvasOf(bottom));
		expect(wait.length).toBeGreaterThanOrEqual(2);
		expect(Math.min(...wait)).toBeGreaterThanOrEqual(65);
		expect(Math.max(...wait)).toBeLessThanOrEqual(90);
	});

	// Kills a pinned readout left over the plot a pinch is moving.
	test("a second finger closes a pinned readout and starts a pinch", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		await tap(over, 0.5);
		await expect.element(close()).toBeInTheDocument();

		await commands.touch("touchStart", [
			finger(over, 1, IN[0][0]),
			finger(over, 2, IN[0][1]),
		]);
		await expect.element(close()).not.toBeInTheDocument();
		await commands.touch("touchEnd", []);
		await pinch(over, ...IN);
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);
	});

	// Kills a third finger that takes over the pinch from one of its two.
	test("a third finger does nothing to a pinch", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		const [from, to] = IN;
		await commands.touch("touchStart", [
			finger(over, 1, from[0]),
			finger(over, 2, from[1]),
		]);
		await commands.touch("touchStart", [
			finger(over, 1, from[0]),
			finger(over, 2, from[1]),
			finger(over, 3, 0.9),
		]);
		for (let step = 1; step <= 4; step++) {
			const part = step / 4;
			await commands.touch("touchMove", [
				finger(over, 1, from[0] + (to[0] - from[0]) * part),
				finger(over, 2, from[1] + (to[1] - from[1]) * part),
				finger(over, 3, 0.9 - 0.6 * part),
			]);
		}
		await commands.touch("touchEnd", []);
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);
	});

	// Kills a pinch that waits for both fingers, goes on with the one left,
	// starts again when a new finger lands beside it, or lets those fingers pin.
	test("a pinch ends when its first finger lifts, and the fingers after it do nothing", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		await spread(over, ...IN);
		// Lifts finger 2 alone; finger 1 stays down at 0.05.
		await commands.touch("touchEnd", [finger(over, 2, IN[1][1])]);
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);

		for (const fraction of [0.3, 0.6]) {
			await commands.touch("touchMove", [finger(over, 1, fraction)]);
		}
		await settle();
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);

		// A new finger beside the one left, then both spread apart.
		await commands.touch("touchStart", [
			finger(over, 1, 0.6),
			finger(over, 3, 0.7),
		]);
		for (const [left, right] of [
			[0.5, 0.8],
			[0.3, 0.95],
		] as const) {
			await commands.touch("touchMove", [
				finger(over, 1, left),
				finger(over, 3, right),
			]);
		}
		await commands.touch("touchEnd", []);
		await settle();
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);
		expect(close().query()).toBeNull();

		await tap(over, 0.5);
		await expect.element(close()).toBeInTheDocument();
	});

	// Kills a hard pinch from 2 points with room around them that stops where
	// it started rather than narrow to them, as a gentler pinch does.
	for (const [xAxis, span] of [
		["date", "Sep 4 to Sep 5"],
		["version", "103 to 104"],
	] as const) {
		test(`a hard pinch from 2 points with room around them narrows to them on the ${xAxis} axis`, async () => {
			mount({ data: RISING, xAxis });
			await settle();
			const over = await plotArea();
			const box = over.getBoundingClientRect();
			// Reports 3 and 4, from 2.6 to 4.4 of 9.
			const at = (report: number) => ({
				position: { x: (box.width * report) / 9, y: box.height - 12 },
			});
			await page.elementLocator(over).hover(at(2.6));
			await commands.mouseDown();
			await page.elementLocator(over).hover(at(4.4));
			await commands.mouseUp();
			await expect.element(zoom()).toBeInTheDocument();

			await pinch(over, [0.45, 0.55], [0.02, 0.98]);
			await expect.element(zoom()).toHaveTextContent(span);
			await page.getByRole("button", { name: "Zoom out" }).click();
			await settle();
			await expect.element(zoom()).toBeInTheDocument();
		});
	}

	// Kills a pinch in at 2 points that pushes the range it started from again,
	// so a Zoom out then changes nothing.
	test("a pinch in at 2 points pushes no step that changes nothing", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		await pinch(over, [0.45, 0.55], [0.02, 0.98]);
		await expect.element(zoom()).toBeInTheDocument();
		await pinch(over, [0.45, 0.55], [0.02, 0.98]);
		await page.getByRole("button", { name: "Zoom out" }).click();
		await settle();
		expect(zoom().query()).toBeNull();
	});

	// Kills a pinch whose fingers sit one above the other, whose spread near
	// nothing would scale the range by near nothing, or by no number at all.
	test("fingers one above the other zoom nothing", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		await commands.touch("touchStart", [
			finger(over, 1, 0.5, -40),
			finger(over, 2, 0.5, 40),
		]);
		for (const nudge of [0.005, 0.01]) {
			await commands.touch("touchMove", [
				finger(over, 1, 0.5 - nudge, -40),
				finger(over, 2, 0.5 + nudge, 40),
			]);
		}
		await commands.touch("touchEnd", []);
		await settle();
		expect(zoom().query()).toBeNull();

		const status = page.getByRole("status");
		over.focus();
		await userEvent.keyboard("{Home}");
		await expect.element(status).toHaveTextContent("10.00 ns");
		await userEvent.keyboard("{End}");
		await expect.element(status).toHaveTextContent("100.00 ns");
	});

	// Kills a pinch's live range kept over data that arrived while the fingers
	// were down, which no lift can then end.
	test("data that arrives mid-pinch draws at every point", async () => {
		const { setData } = mount({ data: RISING });
		await settle();
		await spread(await plotArea(), ...IN);
		setData(
			testData([
				testLine({
					id: "rising",
					y: [11, 21, 31, 41, 51, 61, 71, 81, 91, 101],
				}),
			]),
		);
		await settle();
		await commands.touch("touchEnd", []);
		await settle();
		expect(zoom().query()).toBeNull();

		const over = await plotArea();
		const status = page.getByRole("status");
		over.focus();
		await userEvent.keyboard("{Home}");
		await expect.element(status).toHaveTextContent("11.00 ns");
		await userEvent.keyboard("{End}");
		await expect.element(status).toHaveTextContent("101.00 ns");
	});

	// Kills a pinch the browser cancels that keeps its zoom.
	test("a cancelled pinch leaves the plot as it was", async () => {
		mount({ data: RISING });
		await settle();
		const over = await plotArea();
		await spread(over, ...IN);
		await commands.touch("touchCancel", []);
		await settle();
		expect(zoom().query()).toBeNull();

		const status = page.getByRole("status");
		over.focus();
		await userEvent.keyboard("{Home}");
		await expect.element(status).toHaveTextContent("10.00 ns");
		await userEvent.keyboard("{End}");
		await expect.element(status).toHaveTextContent("100.00 ns");
	});

	// Kills a plot area that keeps a finger's swipe from scrolling the page, a
	// swipe that zooms, or one that leaves its finger behind, which the next
	// pinch would pair with its first.
	test("a 1 finger swipe still scrolls the page and zooms nothing", async () => {
		mount({ data: RISING });
		const below = document.createElement("div");
		below.style.height = "3000px";
		document.body.append(below);
		await settle();
		const over = await plotArea();
		const box = over.getBoundingClientRect();
		const x = box.left + box.width * 0.4;
		const y = box.top + box.height - 10;
		await commands.touch("touchStart", [{ id: 4, x, y }]);
		for (let step = 1; step <= 6; step++) {
			await commands.touch("touchMove", [{ id: 4, x, y: y - 25 * step }]);
		}
		await commands.touch("touchEnd", []);
		await expect
			.poll(() => document.scrollingElement?.scrollTop ?? 0)
			.toBeGreaterThan(0);
		expect(zoom().query()).toBeNull();

		await scrolled();
		document.scrollingElement?.scrollTo(0, 0);
		await scrolled();
		// The browser keeps a touch to itself for a moment after a scroll, so tap until a readout pins.
		await expect
			.poll(
				async () => {
					if (!close().query()) {
						await tap(over, 0.5);
						await new Promise((resolve) => setTimeout(resolve, 400));
					}
					return close().query() !== null;
				},
				{ timeout: 8000 },
			)
			.toBe(true);
		expect(zoom().query()).toBeNull();

		// A pinch pairs its own 2 fingers, with none the swipe left behind.
		await pinch(over, ...IN);
		await expect.element(zoom()).toHaveTextContent(IN_SPAN);
	});
});
