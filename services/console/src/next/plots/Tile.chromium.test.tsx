import "@bencherdev/ui/styles.css";
import "../plot/plot.css";
import "../styles/console.css";
import "./plots.css";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { JsonConsolePlot } from "../../types/bencher";
import { testTile } from "./testing";
import Tile, { drawingOf } from "./Tile";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (tiles: (JsonConsolePlot | undefined)[], failed = false) => {
	// One column, so a grid row stretching its tiles cannot hide a difference.
	const root = document.createElement("div");
	root.style.width = "min(560px, 100%)";
	document.body.append(root);
	dispose = render(
		() =>
			tiles.map((tile, index) => (
				<Tile
					title={`Pin ${index + 1}`}
					tile={tile}
					window="4w"
					failed={failed}
					openHref="#"
					menu={{
						first: index === 0,
						last: index === tiles.length - 1,
						canUnpin: true,
						onRename: () => {},
						onMove: () => {},
						onUnpin: () => {},
					}}
				/>
			)),
		root,
	);
};

const height = (name: string) =>
	page.getByRole("region", { name }).element().getBoundingClientRect().height;

// Kills a tile whose size follows its lines, so tiles move as each batch
// arrives: no lines yet, one line, a full key, and more lines than it shows.
test("a tile holds one size whatever lines it draws", async () => {
	// Screens tall enough that every tile is near enough to draw.
	for (const [width, screen] of [
		[1280, 2600],
		[390, 2600],
	] as const) {
		await page.viewport(width, screen);
		mount([
			undefined,
			testTile({ lines: 1 }),
			testTile({ lines: 4 }),
			testTile({ lines: 9 }),
		]);
		// The three tiles with lines each draw a canvas.
		await expect
			.poll(() => document.querySelectorAll("canvas").length)
			.toBeGreaterThanOrEqual(3);
		const sizes = ["Pin 1", "Pin 2", "Pin 3", "Pin 4"].map(height);
		expect(sizes[0]).toBeGreaterThan(200);
		expect(new Set(sizes).size, `${width}: ${sizes}`).toBe(1);
		dispose?.();
		document.body.replaceChildren();
	}
});

// Kills a menu that offers a move a plot cannot make.
test("a tile's menu offers the moves it can make", async () => {
	mount([testTile(), testTile(), testTile()]);
	const moves = async (name: string) => {
		await page.getByRole("button", { name: `More for ${name}` }).click();
		const menu = page.getByRole("menu", { name: `More for ${name}` });
		const state = await Promise.all(
			["Move up", "Move down", "Move to top", "Move to bottom"].map(
				(item) =>
					(
						menu
							.getByRole("menuitem", { name: item })
							.element() as HTMLButtonElement
					).disabled,
			),
		);
		await page.getByRole("button", { name: `More for ${name}` }).click();
		return state;
	};
	expect(await moves("Pin 1")).toEqual([true, false, true, false]);
	expect(await moves("Pin 2")).toEqual([false, false, false, false]);
	expect(await moves("Pin 3")).toEqual([false, true, false, true]);
});

// Kills a tile that draws below the fold, so a page of tiles builds every plot at once.
test("a tile draws its plot only as it nears the screen", async () => {
	await page.viewport(1280, 900);
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<>
				<div style={{ height: "3000px" }} />
				<Tile title="Far" tile={testTile()} window="4w" openHref="#" />
			</>
		),
		root,
	);
	await new Promise((resolve) => setTimeout(resolve, 100));
	expect(document.querySelectorAll("canvas")).toHaveLength(0);
	page.getByRole("region", { name: "Far" }).element().scrollIntoView();
	await expect
		.poll(() => document.querySelectorAll("canvas").length)
		.toBeGreaterThan(0);
});

// Kills a tile that draws every line of its pin, or focuses none, whatever the
// pin saved.
test("a tile hides and focuses the lines its pin saved", async () => {
	const tile = testTile({ lines: 3 });
	// Each line has limits, so each key entry has a boundary button.
	for (const line of tile.perf.lines) {
		line.series.upper = line.series.y.map((value) =>
			value === null ? null : value + 5,
		);
	}
	const [first, second] = drawingOf(tile).lines;
	tile.plot.hidden = [first?.id ?? ""];
	tile.plot.focus = second?.id ?? "";
	mount([tile]);
	const key = page.getByRole("region", { name: "Pin 1" });
	await expect
		.element(key.getByRole("button", { name: /size=1.*: show its line$/ }))
		.toBeInTheDocument();
	await expect
		.element(key.getByRole("button", { name: /size=2.*: hide its line$/ }))
		.toBeInTheDocument();
	await expect
		.element(key.getByRole("button", { name: /^Hide boundary of .*size=2/ }))
		.toBeInTheDocument();
	await expect
		.element(key.getByRole("button", { name: /^Show boundary of .*size=3/ }))
		.toBeInTheDocument();
});

// Kills a failed tile whose frame still reads as loading inside its figure,
// so a screen reader hears both that it did not load and that it is loading.
test("a tile whose batch failed says so inside its figure, never that it is loading", async () => {
	await page.viewport(1280, 900);
	mount([undefined], true);
	const figure = page.getByRole("figure", { name: "The plot did not load" });
	await expect.element(figure).toBeVisible();
	await expect
		.element(figure.getByRole("img", { name: "The plot did not load" }))
		.toBeVisible();
	expect(
		figure.getByRole("img", { name: "Loading the plot" }).query(),
	).toBeNull();
});
