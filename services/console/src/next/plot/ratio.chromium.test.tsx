import "@bencherdev/ui/styles.css";
import "./plot.css";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import { settle } from "./pixels";
import { START, testData, testLine } from "./testing";

// uPlot reads the pixel ratio as it loads and again when a media query reports a change, so this file sets one before it imports the plot.
const RATIO = 1.5;
Object.defineProperty(window, "devicePixelRatio", {
	value: RATIO,
	configurable: true,
});

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills measuring x labels at the CSS font size, which a fractional ratio rounds up when it draws them, and bounding them by the CSS width, which drops every label across the right part of the canvas.
test("keeps the first x label inside the canvas and labels its right part at a fractional pixel ratio", async () => {
	await page.viewport(1280, 900);
	const { default: Plot } = await import("./Plot");
	const seconds = Array.from({ length: 31 }, (_, index) => index);
	const line = testLine({ id: "line", y: seconds.map((second) => second % 3) });
	const drawn: {
		left: number;
		right: number;
		width: number;
		canvas: number;
	}[] = [];
	const fillText = CanvasRenderingContext2D.prototype.fillText;
	CanvasRenderingContext2D.prototype.fillText = function (
		this: CanvasRenderingContext2D,
		...args: Parameters<typeof fillText>
	) {
		const [text, x] = args;
		if (this.textAlign === "center") {
			const width = this.measureText(text).width;
			drawn.push({
				left: x - width / 2,
				right: x + width / 2,
				width,
				canvas: this.canvas.width,
			});
		}
		return fillText.apply(this, args);
	};
	// The split at the first second sits `shift` ms after the start of the x range.
	const mount = async (shift: number) => {
		dispose?.();
		document.body.replaceChildren();
		drawn.length = 0;
		const root = document.createElement("div");
		root.style.width = "900px";
		document.body.append(root);
		const data = {
			...testData([line]),
			x: seconds.map((second) => START + (second - 30) * 1000 - shift),
		};
		dispose = render(
			() => (
				<Plot
					data={data}
					hidden={[]}
					focused={null}
					onHiddenChange={() => {}}
					onFocusChange={() => {}}
				/>
			),
			root,
		);
		await settle();
		const over = root.querySelector<HTMLElement>("[role=group]");
		const canvas = root.querySelector("canvas");
		if (!over || !canvas) {
			throw new Error("The plot has no area");
		}
		const box = over.getBoundingClientRect();
		return {
			left: box.left - canvas.getBoundingClientRect().left,
			width: box.width,
		};
	};
	try {
		const area = await mount(0);
		const label = drawn[0]?.width ?? 0;
		// One device pixel past the edge once drawn, but inside it measured at 11 px.
		const center = (label / 2 - 1) / RATIO;
		await mount(((center - area.left) / area.width) * 30_000);
	} finally {
		CanvasRenderingContext2D.prototype.fillText = fillText;
	}
	expect(drawn.length).toBeGreaterThanOrEqual(2);
	for (const { left } of drawn) {
		expect(left).toBeGreaterThanOrEqual(0);
	}
	const canvas = drawn[0]?.canvas ?? 0;
	expect(Math.max(...drawn.map(({ right }) => right))).toBeGreaterThan(
		canvas * 0.75,
	);
});
