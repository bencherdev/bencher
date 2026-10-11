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

const mount = (tiles: (JsonConsolePlot | undefined)[]) => {
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
					windowText="saved window 4w"
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
				<Tile
					title="Far"
					tile={testTile()}
					windowText="saved window 4w"
					openHref="#"
				/>
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
		.element(key.getByRole("button", { name: /^Focus .*size=2/ }))
		.toHaveAttribute("aria-pressed", "true");
	await expect
		.element(key.getByRole("button", { name: /^Focus .*size=3/ }))
		.toHaveAttribute("aria-pressed", "false");
});
