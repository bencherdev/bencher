import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "../plots/plots.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonConsolePlot,
	JsonConsolePlots,
	JsonPlot,
} from "../../types/bencher";
import { type Api, ApiError, type Method } from "../api";
import { budget } from "../budget";
import { createQueryClient } from "../cache";
import { NEXT_PROJECTS } from "../paths";
import { middle, rowsOf, tokenRgb } from "../plot/pixels";
import { placed } from "../plots/order";
import { testTile } from "../plots/testing";
import { ProjectContext } from "../project";
import { consoleProjectQuery } from "../queries";
import { bootstrapOf } from "../settings/testing";
import Plots from "./Plots";

const PATH = `${NEXT_PROJECTS}/hashbrown/plots`;

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});
beforeEach(async () => {
	await page.viewport(1280, 720);
	window.scrollTo(0, 0);
});

interface Sent {
	method: Method;
	path: string;
	body: unknown;
}

/**
 * An API holding `count` pins, "Pin 1" on top, saved at `windows` (4w unless given), each drawn over the window it is asked for.
 * A read `held` matches waits until `release`; a change `refused` matches fails.
 */
const plotsApi = (
	count: number,
	windows: readonly number[] = [],
	alerting = false,
) => {
	const pins = new Map(
		Array.from({ length: count }, (_, index) => {
			const tile = testTile({
				uuid: `plot-${index + 1}`,
				title: `Pin ${index + 1}`,
				alerting,
				...(windows[index] === undefined ? {} : { window: windows[index] }),
			});
			return [tile.plot.uuid, tile] as const;
		}),
	);
	let order = [...pins.keys()];
	const reads: URL[] = [];
	const sent: Sent[] = [];
	const waiting: (() => void)[] = [];
	const state = {
		held: (_: URL) => false,
		refused: (_: Sent) => false,
	};
	const answer = (url: URL): JsonConsolePlots => {
		const index = Number(url.searchParams.get("page") ?? 1) - 1;
		const perPage = Number(url.searchParams.get("per_page") ?? 8);
		const end = Number(url.searchParams.get("end_time"));
		const start = url.searchParams.get("start_time");
		return {
			order: order.map((uuid) => ({
				uuid,
				title: pins.get(uuid)?.plot.title ?? "",
			})),
			plots: order
				.slice(index * perPage, (index + 1) * perPage)
				.flatMap((uuid): JsonConsolePlot[] => {
					const tile = pins.get(uuid);
					if (!tile) {
						return [];
					}
					const from =
						start === null ? end - tile.plot.window * 1000 : Number(start);
					return [
						{
							...tile,
							perf: {
								...tile.perf,
								window: { start_time: from, end_time: end, clamped: false },
							},
						},
					];
				}),
		};
	};
	const api: Api = {
		get: async <T,>(path: string) => {
			const url = new URL(path, "http://api.test");
			reads.push(url);
			if (state.held(url)) {
				await new Promise<void>((resolve) => waiting.push(resolve));
			}
			if (url.pathname.endsWith("/v0/console/projects/hashbrown/plots")) {
				return { data: answer(url) as T, headers: new Headers() };
			}
			if (url.pathname.endsWith("/v0/console/projects/hashbrown/perf")) {
				return {
					data: pins.get("plot-1")?.perf as T,
					headers: new Headers(),
				};
			}
			return new Promise<never>(() => {});
		},
		send: async <T,>(method: Method, path: string, body?: unknown) => {
			const change = { method, path, body };
			sent.push(change);
			if (state.refused(change)) {
				throw new ApiError(500, "server", "refused");
			}
			if (path.endsWith("/alerts")) {
				return { data: { changed: 1 } as T, headers: new Headers() };
			}
			const uuid = path.split("/").at(-1) ?? "";
			const plot = pins.get(uuid)?.plot as JsonPlot;
			const index = (body as { index?: number } | undefined)?.index;
			if (method === "PATCH" && index !== undefined) {
				order = placed(order, uuid, index);
			}
			return {
				data: { ...plot, ...(body as object) } as T,
				headers: new Headers(),
			};
		},
	};
	const release = () => {
		for (const resolve of waiting.splice(0)) {
			resolve();
		}
	};
	const plotsReads = () =>
		reads.filter(({ pathname }) =>
			pathname.endsWith("/v0/console/projects/hashbrown/plots"),
		);
	return { api, reads, plotsReads, sent, state, release };
};

const newClient = (api: Api, count: number) => {
	const client = createQueryClient(() => {});
	client.setDefaultOptions({
		queries: { ...client.getDefaultOptions().queries, retry: false },
	});
	client.setQueryData(consoleProjectQuery(api, "hashbrown").queryKey, {
		...bootstrapOf(),
		plots: count,
	});
	return client;
};

const mount = (api: Api, client: QueryClient) => {
	const history = createMemoryHistory();
	history.set({ value: PATH });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<MemoryRouter
					history={history}
					base={NEXT_PROJECTS}
					root={(props) => (
						<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
							<Suspense fallback={<p>Suspended</p>}>{props.children}</Suspense>
						</ProjectContext.Provider>
					)}
				>
					<Route path="/:project/plots" component={Plots} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
};

const titles = () =>
	[...document.querySelectorAll("main .plots-grid h2")].map(
		(heading) => heading.textContent,
	);
const figure = (title: string) =>
	page.getByRole("region", { name: title, exact: true }).getByRole("figure");
/** Pins the readout of `title`'s tile on its alert, the marker at its last point. */
const pinAlert = async (title: string) => {
	const plot = figure(title).getByRole("group", { name: /lines?,/ });
	await expect.element(plot).toBeVisible();
	const over = plot.element() as HTMLElement;
	const canvas = over.parentElement?.querySelector("canvas");
	if (!canvas) {
		throw new Error("The plot area has no canvas");
	}
	const box = over.getBoundingClientRect();
	const x = box.right - 4;
	const marker = () =>
		rowsOf(canvas, x, box.top, box.bottom, tokenRgb("--color-marker"));
	await expect.poll(() => marker().length).toBeGreaterThan(0);
	await page.elementLocator(over).click({
		position: { x: x - box.left, y: middle(marker()) - box.top },
	});
	return figure(title).getByRole("status");
};
const focusedName = () =>
	document.activeElement?.getAttribute("aria-label") ??
	document.activeElement?.textContent;

// Kills a batch that is not one screen of tiles, so the first view waits on
// every pin's lines, and a tile below it that never asks for its own batch.
test("the first batch is a screen of tiles, and the next comes as it nears", async () => {
	const { api, plotsReads } = plotsApi(20);
	mount(api, newClient(api, 20));
	await expect.element(figure("Pin 1")).toHaveAccessibleName("2 lines over 4w");
	expect(
		plotsReads().map(({ searchParams }) => searchParams.get("per_page")),
	).toEqual(["6"]);
	page
		.getByRole("region", { name: "Pin 7", exact: true })
		.element()
		.scrollIntoView();
	await expect.element(figure("Pin 7")).toHaveAccessibleName("2 lines over 4w");
	expect(
		plotsReads().map(({ searchParams }) => [
			searchParams.get("page"),
			searchParams.get("per_page"),
		]),
	).toEqual([
		["1", "6"],
		["2", "6"],
	]);
});

// Kills a page window that drops the tiles drawn for a skeleton, or that does
// not mark them busy, while its answer is on the way.
test("a page window keeps the tiles drawn and busy until its answer", async () => {
	const { api, state, release } = plotsApi(3);
	mount(api, newClient(api, 3));
	await expect.element(figure("Pin 3")).toHaveAccessibleName("2 lines over 4w");
	state.held = (url) => url.searchParams.has("start_time");
	await page.getByRole("radio", { name: "1w" }).click();
	const grid = document.querySelector(".plots-grid");
	await expect.poll(() => grid?.getAttribute("aria-busy")).toBe("true");
	for (const title of ["Pin 1", "Pin 2", "Pin 3"]) {
		await expect.element(figure(title)).toHaveAccessibleName("2 lines over 4w");
	}
	release();
	for (const title of ["Pin 1", "Pin 2", "Pin 3"]) {
		await expect.element(figure(title)).toHaveAccessibleName("2 lines over 1w");
	}
	expect(grid?.getAttribute("aria-busy")).toBe("false");
});

// Kills a page that opens at a page window rather than Per plot, a page window
// left off some tiles or kept for the next view, tiles still tagged with their
// own windows while they draw the page's, and a Per plot that asks again.
test("Per plot draws each tile at its own window, and a page window draws them all for this view only", async () => {
	const DAY = 86_400;
	const { api, plotsReads } = plotsApi(3, [7 * DAY, 28 * DAY, 92 * DAY]);
	const client = newClient(api, 3);
	mount(api, client);
	const windowGroup = page.getByRole("radiogroup", { name: "Window" });
	const own = [
		["Pin 1", "1w"],
		["Pin 2", "4w"],
		["Pin 3", "3m"],
	] as const;
	const drawnAtOwn = async () => {
		for (const [title, window] of own) {
			await expect
				.element(figure(title))
				.toHaveAccessibleName(`2 lines over ${window}`);
			await expect
				.element(
					page
						.getByRole("region", { name: title, exact: true })
						.getByText(window, { exact: true }),
				)
				.toBeVisible();
		}
		await expect
			.element(windowGroup.getByRole("radio", { name: "Per plot" }))
			.toBeChecked();
	};
	await drawnAtOwn();

	await windowGroup.getByRole("radio", { name: "1w" }).click();
	for (const [title, window] of own) {
		await expect.element(figure(title)).toHaveAccessibleName("2 lines over 1w");
		expect(
			page
				.getByRole("region", { name: title, exact: true })
				.getByText(window, { exact: true })
				.elements(),
		).toEqual([]);
	}
	const asked = plotsReads().length;
	await windowGroup.getByRole("radio", { name: "Per plot" }).click();
	await drawnAtOwn();
	expect(plotsReads()).toHaveLength(asked);

	await windowGroup.getByRole("radio", { name: "3m" }).click();
	await expect.element(figure("Pin 1")).toHaveAccessibleName("2 lines over 3m");
	dispose?.();
	document.body.replaceChildren();
	mount(api, client);
	await drawnAtOwn();
});

// Kills an empty Plots whose places to pin from do not lead to their tabs.
test("an empty Plots links to Explore, Runs, and Alerts to pin from", async () => {
	const { api } = plotsApi(0);
	mount(api, newClient(api, 0));
	for (const [name, tab] of [
		["Explore", "explore"],
		["report", "runs"],
		["alert", "alerts"],
	] as const) {
		await expect
			.element(page.getByRole("link", { name, exact: true }))
			.toHaveAttribute("href", `${NEXT_PROJECTS}/hashbrown/${tab}`);
	}
});

// Kills an empty Plots that draws New plot twice, a New plot that moves when
// the first pin arrives, and one that does not open Explore.
test("New plot is drawn once on an empty Plots, where the first pin leaves it, and opens Explore", async () => {
	const newPlot = page.getByRole("link", { name: "New plot" });
	const placed = () => {
		const { x, y, width, height } = newPlot.element().getBoundingClientRect();
		return { x, y, width, height };
	};
	const empty = plotsApi(0);
	mount(empty.api, newClient(empty.api, 0));
	await expect
		.element(page.getByRole("link", { name: "report", exact: true }))
		.toBeVisible();
	expect(newPlot.elements()).toHaveLength(1);
	await expect
		.element(newPlot)
		.toHaveAttribute("href", `${NEXT_PROJECTS}/hashbrown/explore`);
	const before = placed();
	dispose?.();
	document.body.replaceChildren();

	const one = plotsApi(1);
	mount(one.api, newClient(one.api, 1));
	await expect.element(figure("Pin 1")).toHaveAccessibleName("2 lines over 4w");
	expect(placed()).toEqual(before);
});

// Kills a confirmed move kept only in the page, so coming back draws the old
// order from the cache until the API answers again.
test("a move is in the cache when the page is opened again", async () => {
	const { api, sent, state } = plotsApi(3);
	const client = newClient(api, 3);
	mount(api, client);
	await expect.element(figure("Pin 1")).toHaveAccessibleName("2 lines over 4w");
	await page.getByRole("button", { name: "More for Pin 1" }).click();
	await page.getByRole("menuitem", { name: "Move down" }).click();
	await expect.poll(() => sent.length).toBe(1);
	await expect.poll(titles).toEqual(["Pin 2", "Pin 1", "Pin 3"]);
	dispose?.();
	document.body.replaceChildren();

	state.held = () => true;
	mount(api, client);
	await expect.poll(titles).toEqual(["Pin 2", "Pin 1", "Pin 3"]);
});

// Kills a Reorder move that drops the focus to the page, so the next key press
// does nothing, and a row's button left enabled for a move it cannot make.
test("a Reorder move keeps the focus on the pin's move buttons", async () => {
	const { api, sent } = plotsApi(3);
	mount(api, newClient(api, 3));
	await expect.element(figure("Pin 3")).toHaveAccessibleName("2 lines over 4w");
	await page.getByRole("button", { name: "Reorder" }).click();
	const reorder = page.getByRole("region", { name: "Reorder" });
	await expect
		.element(reorder.getByRole("button", { name: "Move Pin 3 down" }))
		.toBeDisabled();
	(
		reorder
			.getByRole("button", { name: "Move Pin 3 up" })
			.element() as HTMLElement
	).focus();

	await userEvent.keyboard("{Enter}");
	await expect.poll(() => sent.length).toBe(1);
	expect(focusedName()).toBe("Move Pin 3 up");
	await userEvent.keyboard("{Enter}");
	await expect.poll(() => sent.length).toBe(2);
	// At the top its up button is disabled, so the focus takes the other one.
	expect(focusedName()).toBe("Move Pin 3 down");
	await expect
		.element(reorder.getByRole("button", { name: "Move Pin 3 up" }))
		.toBeDisabled();
	await expect
		.element(reorder.getByRole("button", { name: "Move Pin 2 down" }))
		.toBeDisabled();
	expect(sent.map(({ body }) => body)).toEqual([{ index: 1 }, { index: 0 }]);
});

// Kills a move from a tile's menu that drops the focus to the page, whether
// the API takes the move or refuses it and the tile goes back.
test("a move from a tile's menu returns the focus to its More button", async () => {
	const { api, sent, state } = plotsApi(3);
	mount(api, newClient(api, 3));
	await expect.element(figure("Pin 3")).toHaveAccessibleName("2 lines over 4w");
	const more = (title: string) =>
		page
			.getByRole("button", { name: `More for ${title}` })
			.element() as HTMLElement;

	more("Pin 2").focus();
	await userEvent.keyboard("{Enter}");
	await expect
		.element(page.getByRole("menu", { name: "More for Pin 2" }))
		.toBeVisible();
	await userEvent.keyboard("{ArrowDown}{Enter}");
	await expect.poll(titles).toEqual(["Pin 2", "Pin 1", "Pin 3"]);
	expect(focusedName()).toBe("More for Pin 2");
	await expect.poll(() => sent.length).toBe(1);

	state.refused = () => true;
	more("Pin 3").focus();
	await userEvent.keyboard("{Enter}");
	await expect
		.element(page.getByRole("menu", { name: "More for Pin 3" }))
		.toBeVisible();
	// Rename, Move up, Move to top.
	await userEvent.keyboard("{ArrowDown}{ArrowDown}{Enter}");
	await expect
		.element(page.getByText("Bencher did not move the plot"))
		.toBeVisible();
	expect(titles()).toEqual(["Pin 2", "Pin 1", "Pin 3"]);
	expect(focusedName()).toBe("More for Pin 3");
});

// Kills a pin dialog that asks the API again for the lines its tile already draws.
test("Rename previews the lines its tile already draws", async () => {
	const { api, reads } = plotsApi(3);
	mount(api, newClient(api, 3));
	await expect.element(figure("Pin 2")).toHaveAccessibleName("2 lines over 4w");
	await page.getByRole("button", { name: "More for Pin 2" }).click();
	await page.getByRole("menuitem", { name: "Rename" }).click();
	const dialog = page.getByRole("dialog", { name: "Edit pin" });
	await expect
		.element(dialog.getByRole("figure"))
		.toHaveAccessibleName(/^2 lines over /);
	expect(
		reads.filter(({ pathname }) =>
			pathname.endsWith("/v0/console/projects/hashbrown/perf"),
		),
	).toEqual([]);
});

// One frame at 120 Hz, and the most a whole-page mode switch may take.
const FRAME_MS = 8.3;
const SWITCH_MS = 50;
const RUNS = 9;

const median = (values: number[]) =>
	[...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ??
	Number.NaN;

/**
 * The main thread's work for a click: its handlers, styles, and layout, then the
 * next frame's through its resize observers and paint, without the idle wait between.
 */
const timeClick = async (element: HTMLElement) => {
	const start = performance.now();
	element.click();
	document.body.getBoundingClientRect();
	const handled = performance.now();
	const sentinel = document.createElement("div");
	document.body.append(sentinel);
	const framed = await new Promise<number>((resolve) =>
		requestAnimationFrame((frame) => {
			const begun = Math.max(frame, handled);
			// Created last, it is called after every observer the click woke.
			const observer = new ResizeObserver(() => {
				observer.disconnect();
				const channel = new MessageChannel();
				channel.port1.onmessage = () => resolve(performance.now() - begun);
				channel.port2.postMessage(undefined);
			});
			observer.observe(sentinel);
		}),
	);
	sentinel.remove();
	return handled - start + framed;
};

const button = (name: string) =>
	page.getByRole("button", { name, exact: true }).element() as HTMLElement;

const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));

/** 64 pins, every tile on the first screen drawn. */
const full = async () => {
	const pins = plotsApi(64);
	mount(pins.api, newClient(pins.api, 64));
	await expect.element(figure("Pin 1")).toHaveAccessibleName("2 lines over 4w");
	await expect.element(figure("Pin 4")).toHaveAccessibleName("2 lines over 4w");
	await frame();
	return pins;
};

const tiles = () =>
	page.getByRole("region", { name: /^Pin \d+$/ }).elements() as HTMLElement[];

/** Every batch asked for and every tile drawn, as after scrolling the whole page. */
const drawEvery = async () => {
	// A screen as tall as the page brings every tile near at once.
	await page.viewport(1280, 15_000);
	await expect
		.poll(() => tiles().filter((tile) => !tile.querySelector("canvas")), {
			interval: 10,
			timeout: 5_000,
		})
		.toEqual([]);
	await page.viewport(1280, 720);
	window.scrollTo(0, 0);
	await frame();
};

/** Enter Reorder mode, move a pin, and leave, `RUNS` times, `before` each; the median of each. */
const reorderSessions = async (before?: () => Promise<void>) => {
	const enters: number[] = [];
	const moves: number[] = [];
	const leaves: number[] = [];
	for (let run = 0; run < RUNS; run++) {
		await before?.();
		enters.push(await timeClick(button("Reorder")));
		await expect
			.element(page.getByRole("region", { name: "Reorder" }))
			.toBeVisible();
		await frame();
		const name = run % 2 === 0 ? "Move Pin 2 up" : "Move Pin 2 down";
		moves.push(await timeClick(button(name)));
		leaves.push(await timeClick(button("Done")));
		await expect.element(button("Reorder")).toBeVisible();
		await frame();
	}
	return { enter: median(enters), move: median(moves), leave: median(leaves) };
};

// Kills a Reorder mode that takes more than 50 ms to enter or leave at 64 pins,
// and a move whose own work takes more than one 120 Hz frame, whether only the
// first screen of tiles is drawn or every tile is.
test("at 64 pins Reorder mode enters and leaves within 50 ms and moves within a frame", async () => {
	await full();
	const first = await reorderSessions();
	// Done builds the grid again and draws only the tiles near the screen.
	const every = await reorderSessions(drawEvery);
	for (const [state, times] of [
		["first screen drawn", first],
		["every tile drawn", every],
	] as const) {
		console.log(
			`speed plots reorder, ${state}: enter median ${times.enter.toFixed(2)} ms, move ${times.move.toFixed(2)} ms, leave ${times.leave.toFixed(2)} ms`,
		);
		expect(times.enter, state).toBeLessThan(budget(SWITCH_MS));
		expect(times.move, state).toBeLessThan(budget(FRAME_MS));
		expect(times.leave, state).toBeLessThan(budget(SWITCH_MS));
	}
});

// Kills a tile menu whose opening takes more than one 120 Hz frame on a full page.
test("at 64 pins a tile's menu opens within a frame", async () => {
	await full();
	const opens: number[] = [];
	for (let run = 0; run < RUNS; run++) {
		opens.push(await timeClick(button("More for Pin 1")));
		await expect
			.element(page.getByRole("menu", { name: "More for Pin 1" }))
			.toBeVisible();
		await userEvent.keyboard("{Escape}");
		await frame();
	}
	console.log(`speed plots menu: open median ${median(opens).toFixed(2)} ms`);
	expect(median(opens)).toBeLessThan(budget(FRAME_MS));
});

/** Move Pin 3 down a place or back up from its menu, `RUNS` times, `before` each; the median move. */
const menuMoves = async (sent: Sent[], before?: () => Promise<void>) => {
	const moves: number[] = [];
	for (let run = 0; run < RUNS; run++) {
		await before?.();
		const changes = sent.length;
		const down = titles().indexOf("Pin 3") === 2;
		button("More for Pin 3").click();
		const item = page.getByRole("menuitem", {
			name: down ? "Move down" : "Move up",
		});
		await expect.element(item).toBeVisible();
		await frame();
		moves.push(await timeClick(item.element() as HTMLElement));
		await expect.poll(() => sent.length).toBe(changes + 1);
		await expect.poll(() => titles().indexOf("Pin 3")).toBe(down ? 3 : 2);
		await frame();
	}
	return median(moves);
};

// Kills a move from a tile's menu whose own work takes more than one 120 Hz
// frame at 64 pins, whether only the first screen of tiles is drawn or every tile is.
test("at 64 pins a tile's menu moves its pin within a frame", async () => {
	const { sent } = await full();
	const first = await menuMoves(sent);
	const every = await menuMoves(sent, drawEvery);
	for (const [state, move] of [
		["first screen drawn", first],
		["every tile drawn", every],
	] as const) {
		console.log(
			`speed plots menu move, ${state}: median ${move.toFixed(2)} ms`,
		);
		expect(move, state).toBeLessThan(budget(FRAME_MS));
	}
});

// Kills a tile whose plot reads the page's order, so one move from a menu
// redraws every drawn plot on the page.
test("a move from a tile's menu redraws no plot", async () => {
	const { sent } = await full();
	await drawEvery();
	await page
		.getByRole("button", { name: "More for Pin 3", exact: true })
		.click();
	const cleared = new Set<HTMLCanvasElement>();
	const context = CanvasRenderingContext2D.prototype;
	const { clearRect } = context;
	context.clearRect = function (this: CanvasRenderingContext2D, ...args) {
		cleared.add(this.canvas);
		clearRect.apply(this, args);
	};
	try {
		await page.getByRole("menuitem", { name: "Move down" }).click();
		await expect.poll(() => sent.length).toBe(1);
		await expect.poll(() => titles().indexOf("Pin 3")).toBe(3);
		await frame();
		await frame();
	} finally {
		context.clearRect = clearRect;
	}
	const redrawn = tiles().filter((tile) =>
		[...tile.querySelectorAll("canvas")].some((canvas) => cleared.has(canvas)),
	);
	expect(redrawn.map((tile) => tile.querySelector("h2")?.textContent)).toEqual(
		[],
	);
});

// Kills a viewer offered a Dismiss in a tile's pinned readout, which the API
// refuses, and an editor's refused Dismiss that says nothing where it was made.
test("a tile's pinned readout offers Dismiss only to an editor, and says when it did not go through", async () => {
	const viewer = plotsApi(1, [], true);
	const client = newClient(viewer.api, 1);
	client.setQueryData(consoleProjectQuery(viewer.api, "hashbrown").queryKey, {
		...bootstrapOf({ edit: false }),
		plots: 1,
	});
	mount(viewer.api, client);
	const read = await pinAlert("Pin 1");
	await expect
		.element(read.getByRole("link", { name: /^Alert: / }))
		.toBeVisible();
	expect(
		read.getByRole("button", { name: /the alert on/ }).elements(),
	).toHaveLength(0);
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();

	const editor = plotsApi(1, [], true);
	editor.state.refused = ({ path }) => path.endsWith("/alerts");
	mount(editor.api, newClient(editor.api, 1));
	const readout = await pinAlert("Pin 1");
	const dismiss = readout.getByRole("button", {
		name: /^Dismiss the alert on/,
	});
	await dismiss.click();
	await expect.element(readout.getByRole("alert")).toBeVisible();
	await expect.element(dismiss).toBeVisible();
});
