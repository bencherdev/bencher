import "@bencherdev/ui/styles.css";
import "../plot/plot.css";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import Plot from "../plot/Plot";
import type { PlotLayout } from "../plot/types";
import PlotFrame from "./PlotFrame";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** The heights of the plot waiting for its data and of the frame held before its code. */
const heights = (
	reserve: { lines: number; measures: number },
	layout: PlotLayout,
	loading: boolean,
) => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<>
				<section aria-label="Plot">
					<Plot
						data={undefined}
						hidden={[]}
						onHiddenChange={() => {}}
						onFocusChange={() => {}}
						layout={layout}
						reserve={reserve}
					/>
				</section>
				<section aria-label="Frame">
					<PlotFrame reserve={reserve} layout={layout} loading={loading} />
				</section>
			</>
		),
		root,
	);
	const height = (name: string) =>
		page.getByRole("region", { name }).element().getBoundingClientRect().height;
	return { plot: height("Plot"), frame: height("Frame") };
};

// Kills a frame that moves the page when the plot's code replaces it: a panel
// too few or too many, one measure stacked, a key reserving other rows than
// the plot, or a narrow key that folds at another count.
test.each([
	["one line", { lines: 1, measures: 1 }, "dual", true],
	["one measure asked to stack", { lines: 3, measures: 1 }, "stacked", true],
	["a full key", { lines: 14, measures: 2 }, "dual", true],
	["three stacked measures", { lines: 5, measures: 3 }, "stacked", true],
	["a refused query", { lines: 8, measures: 2 }, "stacked", false],
] as const)(
	"the frame holds the plot's size with %s",
	async (_, reserve, layout, loading) => {
		for (const [width, height] of [
			[1280, 900],
			[390, 844],
		] as const) {
			await page.viewport(width, height);
			const { plot, frame } = heights(reserve, layout, loading);
			expect(frame).toBeGreaterThan(0);
			expect(frame).toBe(plot);
			dispose?.();
			document.body.replaceChildren();
		}
	},
);
