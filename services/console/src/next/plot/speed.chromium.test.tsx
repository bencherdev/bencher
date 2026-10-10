import "@bencherdev/ui/styles.css";
import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { createSignal, For } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import { budget } from "../budget";
import CEILINGS from "./ceilings.json";
import InlineHistory from "./InlineHistory";
import Plot from "./Plot";
import { DAY, START, testLine } from "./testing";
import type { PlotData, PlotLine } from "./types";

// Speed ratchets: the median of several runs must stay under its ceiling in ceilings.json scaled by `budget`, and a ceiling only moves down.

const RUNS = 9;

let dispose: (() => void) | undefined;

beforeEach(async () => {
	await page.viewport(1440, 900);
	document.documentElement.setAttribute(THEME_ATTRIBUTE, "dark");
});

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const random = (seed: number) => {
	let state = seed;
	return () => {
		state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
		return state / 4294967296;
	};
};

/** A year of daily reports for `count` lines, half of them under a threshold and some alerting. */
const yearOf = (count: number): PlotData => {
	const points = 365;
	const next = random(42);
	const x = Array.from({ length: points }, (_, index) => START + index * DAY);
	const lines: PlotLine[] = Array.from({ length: count }, (_, index) => {
		const base = 2000 * 1.07 ** index;
		const y = x.map(() => base * (1 + (next() - 0.5) * 0.04));
		const checked = index % 2 === 0;
		return testLine({
			id: `line-${index}`,
			parameters: { threads: index },
			alerting: index % 8 === 7,
			y,
			...(checked && { upper: y.map(() => base * 1.03) }),
			alerts: index % 8 === 7 ? [points - 1] : [],
		});
	});
	return {
		x,
		report: x.map((_, index) => index),
		reports: x.map((_, index) => ({
			uuid: `report-${index}`,
			version: index,
			hash: "a1b2c3d",
		})),
		measures: [{ name: "Latency", units: "nanoseconds (ns)" }],
		lines,
	};
};

const median = (values: number[]) => {
	const sorted = [...values].sort((a, b) => a - b);
	return sorted[Math.floor(sorted.length / 2)] ?? Number.NaN;
};

/**
 * Lets uPlot's queued draw land, lays the page out, and rasterizes every canvas under `root`,
 * so a run counts the main thread's whole share of a frame without waiting for one.
 */
const land = async (root: HTMLElement) => {
	await Promise.resolve();
	root.getBoundingClientRect();
	for (const canvas of root.querySelectorAll("canvas")) {
		canvas.getContext("2d")?.getImageData(0, 0, 1, 1);
	}
};

const mountYear = (data: PlotData) => {
	const root = document.createElement("div");
	root.style.width = "1370px";
	document.body.append(root);
	const [hidden, setHidden] = createSignal<string[]>([]);
	dispose = render(
		() => (
			<Plot
				data={data}
				hidden={hidden()}
				focused={null}
				onHiddenChange={setHidden}
				onFocusChange={() => {}}
			/>
		),
		root,
	);
	return root;
};

const expectUnder = (name: string, runs: number[], ceiling: number) => {
	const value = median(runs);
	const limit = budget(ceiling);
	console.info(
		`speed ${name}: median ${value.toFixed(2)} ms over ${runs.length} runs, budget ${limit.toFixed(2)} ms`,
	);
	expect(value).toBeLessThan(limit);
};

const DATA = yearOf(64);

test("mounts and draws 64 lines of 365 points", async () => {
	const runs: number[] = [];
	for (let run = 0; run < RUNS; run++) {
		const start = performance.now();
		const root = mountYear(DATA);
		await land(root);
		runs.push(performance.now() - start);
		dispose?.();
		dispose = undefined;
		root.remove();
	}
	expectUnder("mount", runs, CEILINGS.mount_64_lines_365_points_ms);
});

test("moves the readout and the hover focus for one pointer move", async () => {
	const root = mountYear(DATA);
	await land(root);
	const over = page.getByRole("group", { name: /64 lines/ }).element();
	const box = over.getBoundingClientRect();
	const runs: number[] = [];
	for (let run = 0; run < RUNS * 3; run++) {
		const event = new PointerEvent("pointermove", {
			clientX: box.left + ((run % 7) + 1) * (box.width / 9),
			clientY: box.top + ((run % 5) + 1) * (box.height / 7),
			pointerType: "mouse",
			bubbles: true,
		});
		const start = performance.now();
		over.dispatchEvent(event);
		await land(root);
		runs.push(performance.now() - start);
	}
	expect(page.getByRole("status").element().textContent).toContain("µs");
	expectUnder("hover", runs, CEILINGS.hover_ms);
});

test("hides and shows a line from the key", async () => {
	const root = mountYear(DATA);
	await land(root);
	const runs: number[] = [];
	// The widest line, alerting so its key entry is in view, and each toggle rescales the axis and rebuilds every path.
	const name = "blake3 threads=63";
	for (let run = 0; run < RUNS * 2; run++) {
		const button = page
			.getByRole("button", { name: new RegExp(`^(Hide|Show) ${name}$`) })
			.element() as HTMLButtonElement;
		const start = performance.now();
		button.click();
		await land(root);
		runs.push(performance.now() - start);
	}
	expectUnder("toggle", runs, CEILINGS.key_toggle_ms);
});

test("draws 200 inline histories", async () => {
	const x = DATA.x.slice(-28);
	const histories = DATA.lines
		.concat(DATA.lines, DATA.lines, DATA.lines)
		.slice(0, 200)
		.map((line) => ({
			y: line.y.slice(-28),
			...(line.upper && { upper: line.upper.slice(-28) }),
			alerts: line.alerts.length ? [27] : [],
		}));
	const runs: number[] = [];
	for (let run = 0; run < RUNS; run++) {
		const root = document.createElement("div");
		document.body.append(root);
		const start = performance.now();
		dispose = render(
			() => (
				<For each={histories}>
					{(line) => <InlineHistory x={x} line={line} label="History" />}
				</For>
			),
			root,
		);
		await land(root);
		runs.push(performance.now() - start);
		dispose();
		dispose = undefined;
		root.remove();
	}
	expectUnder("inline", runs, CEILINGS.inline_200_ms);
});
