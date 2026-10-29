import type { Page } from "@playwright/test";
import { createProject, listPlots, postReport } from "./api";
import { axeInBothThemes } from "./axe";
import { dimensions, explore } from "./explore";
import { expect, consolePath, seed, signedIn, test } from "./fixtures";
import { createPin, pinnedProject } from "./plots";

const { hashbrown } = seed.projects;
const PLOTS = consolePath(hashbrown.slug, "plots");
const DAY = 24 * 60 * 60 * 1000;

/** The seed's pins, top first, with what each draws at its own window. */
const SEEDED = [
	["blake3 input_bytes=65536 threads=1, Latency on main", "2 lines over 4w"],
	["xxh3, Latency on main", "4 lines over 1w"],
	["crc32c, Latency and Throughput on main", "4 lines over 3m"],
] as const;

const titles = (page: Page) =>
	page.getByRole("main").getByRole("heading", { level: 2 });
const tile = (page: Page, title: string) =>
	page.getByRole("region", { name: title, exact: true });
const figure = (page: Page, title: string) =>
	tile(page, title).getByRole("figure");
const more = async (page: Page, title: string, item: string) => {
	await page.getByRole("button", { name: `More for ${title}` }).click();
	await page
		.getByRole("menu", { name: `More for ${title}` })
		.getByRole("menuitem", { name: item })
		.click();
};
const pinTitles = async (
	request: Parameters<typeof listPlots>[0],
	slug: string,
) => (await listPlots(request, slug)).map(({ title }) => title);
const plotsRequests = (page: Page) => {
	const urls: URL[] = [];
	page.on("request", (request) => {
		if (/\/v0\/console\/projects\/[^/]+\/plots/.test(request.url())) {
			urls.push(new URL(request.url()));
		}
	});
	return urls;
};

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

// Kills tiles that ask for their lines one request each, a page in another
// order than the pins', and a tile drawn at another window than its own.
test("the pinned plots draw in one request, each at its own window", async ({
	page,
}) => {
	const requests = plotsRequests(page);
	await page.goto(PLOTS);
	await expect(
		page.getByRole("heading", { level: 1, name: "3 pinned plots" }),
	).toBeVisible();
	await expect(titles(page)).toHaveText(SEEDED.map(([title]) => title));
	for (const [title, drawn] of SEEDED) {
		await expect(figure(page, title)).toHaveAccessibleName(drawn);
	}
	await expect(
		tile(page, SEEDED[0][0]).getByRole("img", { name: "has an alert" }),
	).toBeVisible();
	expect(requests).toHaveLength(1);
	expect(requests[0]?.searchParams.get("start_time")).toBeNull();
	expect(requests[0]?.searchParams.get("end_time")).toBe(
		String(Date.parse(seed.now)),
	);
});

// Kills a page window that redraws only some tiles, one that is saved, and a
// Per plot that asks the API again for the plots' own windows.
test("a page window redraws every tile for this view only", async ({
	page,
}) => {
	const requests = plotsRequests(page);
	await page.goto(PLOTS);
	await expect(figure(page, SEEDED[2][0])).toHaveAccessibleName(SEEDED[2][1]);
	const window = page.getByRole("radiogroup", { name: "Window" });
	const checked = window.getByRole("radio", { checked: true });
	await expect(checked).toHaveAccessibleName("Per plot");
	await window.getByText("1w", { exact: true }).click();
	await expect(checked).toHaveAccessibleName("1w");
	await expect(figure(page, SEEDED[0][0])).toHaveAccessibleName(
		"2 lines over 1w",
	);
	await expect(figure(page, SEEDED[2][0])).toHaveAccessibleName(
		"4 lines over 1w",
	);
	const now = Date.parse(seed.now);
	expect(requests.at(-1)?.searchParams.get("start_time")).toBe(
		String(now - 7 * DAY),
	);

	const asked = requests.length;
	await window.getByText("Per plot", { exact: true }).click();
	for (const [title, drawn] of SEEDED) {
		await expect(figure(page, title)).toHaveAccessibleName(drawn);
	}
	expect(requests).toHaveLength(asked);

	await window.getByText("3m", { exact: true }).click();
	await expect(figure(page, SEEDED[1][0])).toHaveAccessibleName(
		"4 lines over 3m",
	);
	await page.reload();
	for (const [title, drawn] of SEEDED) {
		await expect(figure(page, title)).toHaveAccessibleName(drawn);
	}
	await expect(checked).toHaveAccessibleName("Per plot");
});

// Kills a move the API never hears of or places elsewhere, a rename that is
// not saved, and an Undo that leaves the pin unpinned or puts it back elsewhere.
test("rename, move, and unpin with Undo save as they show", async ({
	page,
	request,
}) => {
	const project = await pinnedProject(request);
	await page.goto(consolePath(project.slug, "plots"));
	await expect(titles(page)).toHaveText(["Pin 1", "Pin 2", "Pin 3"]);

	await more(page, "Pin 1", "Move down");
	await expect(titles(page)).toHaveText(["Pin 2", "Pin 1", "Pin 3"]);
	await expect
		.poll(() => pinTitles(request, project.slug))
		.toEqual(["Pin 2", "Pin 1", "Pin 3"]);
	await more(page, "Pin 2", "Move to bottom");
	await expect(titles(page)).toHaveText(["Pin 1", "Pin 3", "Pin 2"]);

	await more(page, "Pin 3", "Rename");
	const dialog = page.getByRole("dialog", { name: "Edit pin" });
	await dialog.getByRole("textbox", { name: "Title" }).fill("Renamed");
	await dialog.getByRole("button", { name: "Save pin" }).click();
	await expect(dialog).toHaveCount(0);
	await expect(titles(page)).toHaveText(["Pin 1", "Renamed", "Pin 2"]);

	await more(page, "Renamed", "Unpin");
	await expect(titles(page)).toHaveText(["Pin 1", "Pin 2"]);
	const unpinned = page.getByRole("status").filter({ hasText: "Unpinned" });
	await expect(unpinned).toContainText('Unpinned "Renamed"');
	await expect
		.poll(() => pinTitles(request, project.slug))
		.toEqual(["Pin 1", "Pin 2"]);
	await unpinned.getByRole("button", { name: "Undo" }).click();
	await expect(titles(page)).toHaveText(["Pin 1", "Renamed", "Pin 2"]);
	await expect
		.poll(() => pinTitles(request, project.slug))
		.toEqual(["Pin 1", "Renamed", "Pin 2"]);

	await page.reload();
	await expect(titles(page)).toHaveText(["Pin 1", "Renamed", "Pin 2"]);
});

// Kills a Reorder mode whose moves are not saved, announce nothing, or come
// back in the old order.
test("Reorder mode moves a tile up and the order survives a reload", async ({
	page,
	request,
}) => {
	const project = await pinnedProject(request);
	await page.goto(consolePath(project.slug, "plots"));
	await page.getByRole("button", { name: "Reorder" }).click();
	const reorder = page.getByRole("region", { name: "Reorder" });
	await reorder.getByRole("button", { name: "Move Pin 3 up" }).click();
	await expect(page.getByText("Moved Pin 3 up: 2 of 3")).toBeVisible();
	await expect(
		reorder.getByRole("button", { name: "Move Pin 1 up" }),
	).toBeDisabled();
	await page.getByRole("button", { name: "Done" }).click();
	await expect(titles(page)).toHaveText(["Pin 1", "Pin 3", "Pin 2"]);
	await expect
		.poll(() => pinTitles(request, project.slug))
		.toEqual(["Pin 1", "Pin 3", "Pin 2"]);
	await page.reload();
	await expect(titles(page)).toHaveText(["Pin 1", "Pin 3", "Pin 2"]);
});

// Kills a New plot that opens anything but a blank Explore.
test("New plot opens a blank Explore", async ({ page }) => {
	await page.goto(PLOTS);
	await page.getByRole("link", { name: "New plot" }).click();
	await expect(page).toHaveURL(consolePath(hashbrown.slug, "explore"));
	await expect(
		page.getByRole("heading", { level: 1, name: "Unsaved plot" }),
	).toBeVisible();
});

// Kills a dialog whose title, window, or position the tile does not follow,
// or that the API does not keep.
test("the pin dialog changes the title, the window, and the position", async ({
	page,
	request,
}) => {
	const project = await pinnedProject(request);
	await page.goto(consolePath(project.slug, "plots"));
	await more(page, "Pin 1", "Rename");
	const dialog = page.getByRole("dialog", { name: "Edit pin" });
	await dialog.getByRole("textbox", { name: "Title" }).fill("Fortnight");
	await dialog.getByRole("radio", { name: "Custom" }).click();
	await dialog
		.getByRole("spinbutton", { name: "How many days or weeks" })
		.fill("2");
	await page.keyboard.press("Tab");
	await dialog
		.getByRole("combobox", { name: "Days or weeks" })
		.selectOption("weeks");
	await dialog.getByRole("radio", { name: "After" }).click();
	await dialog
		.getByRole("combobox", { name: "After which plot" })
		.selectOption("Pin 3");
	await expect(dialog.getByText("2 weeks", { exact: true })).toBeVisible();
	await dialog.getByRole("button", { name: "Save pin" }).click();

	await expect(titles(page)).toHaveText(["Pin 2", "Pin 3", "Fortnight"]);
	await expect(
		tile(page, "Fortnight").getByText("2 weeks", { exact: true }),
	).toBeVisible();
	await expect(figure(page, "Fortnight")).toHaveAccessibleName(
		"2 lines over 2 weeks",
	);
	const [, , saved] = await listPlots(request, project.slug);
	expect(saved?.title).toBe("Fortnight");
	expect(saved?.window).toBe(14 * 24 * 60 * 60);
	await page.reload();
	await expect(titles(page)).toHaveText(["Pin 2", "Pin 3", "Fortnight"]);
});

// Kills a pin confirmation in Explore with no way to edit the pin.
test("Edit on Explore's pin confirmation opens the pin dialog", async ({
	page,
	request,
}) => {
	const project = await createProject(request);
	await postReport(request, project.slug, {
		start: "2026-09-10T00:00:00Z",
		results: {
			hash: [{ parameters: { size: 1 }, measures: { latency: { value: 1 } } }],
		},
	});
	const id = await dimensions(request, project.slug);
	await page.goto(
		explore(project.slug, {
			branches: [{ uuid: id.branch("main") }],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: [id.benchmark("hash")],
			measures: [id.measure("Latency")],
		}),
	);
	await page.getByRole("button", { name: "Pin to Plots" }).click();
	const confirmation = page.getByRole("status").filter({ hasText: "Pinned" });
	await confirmation.getByRole("button", { name: "Edit" }).click();
	const dialog = page.getByRole("dialog", { name: "Edit pin" });
	await dialog.getByRole("textbox", { name: "Title" }).fill("Edited");
	await dialog.getByRole("radio", { name: "1w" }).click();
	await dialog.getByRole("button", { name: "Save pin" }).click();
	await expect(
		page.getByRole("heading", { level: 1, name: "Edited" }),
	).toBeVisible();
	await expect(confirmation).toContainText('"Edited" · 1w');
	const [pin] = await listPlots(request, project.slug);
	expect(pin?.title).toBe("Edited");
	expect(pin?.window).toBe(7 * 24 * 60 * 60);
});

// Kills a full project that still offers Pin, a limit the page does not show,
// and an API that takes a sixty-fifth pin.
test("at 64 pins Plots says so, Pin says so, and the API refuses another", async ({
	page,
	request,
}) => {
	const project = await pinnedProject(request, 64);
	await page.goto(consolePath(project.slug, "plots"));
	await expect(page.getByText("64 of 64 pinned")).toBeVisible();
	await expect(
		createPin(request, project.slug, { title: "One more" }),
	).rejects.toThrow(/: 400 /);

	const id = await dimensions(request, project.slug);
	await page.goto(
		explore(project.slug, {
			branches: [{ uuid: id.branch("main") }],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: [id.benchmark("hash")],
			measures: [id.measure("Latency")],
		}),
	);
	await expect(
		page.getByRole("button", { name: "Plots is full" }),
	).toBeDisabled();
	await expect(page.getByRole("button", { name: "Pin to Plots" })).toHaveCount(
		0,
	);
});

test.describe("a taller screen", () => {
	test.use({ viewport: { width: 1280, height: 900 } });

	// Kills tiles that draw only once on screen, so scrolling meets empty
	// frames, and tiles that draw far below it.
	test("tiles draw as they near the screen", async ({ page, request }) => {
		const project = await pinnedProject(request, 8);
		await page.goto(consolePath(project.slug, "plots"));
		const canvases = (title: string) => tile(page, title).locator("canvas");
		// The third row sits under the fold, within half a screen of it.
		await expect(canvases("Pin 5").first()).toBeAttached();
		expect((await tile(page, "Pin 5").boundingBox())?.y).toBeGreaterThan(900);
		await expect(canvases("Pin 7")).toHaveCount(0);
		await tile(page, "Pin 7").scrollIntoViewIfNeeded();
		await expect(canvases("Pin 7").first()).toBeAttached();
	});
});

// Kills a pin from Explore that leaves the count of pins behind, and a Save as
// new still offered once Plots is full.
test("pinning the 64th plot in Explore fills Plots at once", async ({
	page,
	request,
}) => {
	const project = await pinnedProject(request, 63);
	const id = await dimensions(request, project.slug);
	await page.goto(
		explore(project.slug, {
			branches: [{ uuid: id.branch("main") }],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: [id.benchmark("hash")],
			measures: [id.measure("Latency")],
		}),
	);
	await page.getByRole("button", { name: "Pin to Plots" }).click();
	await expect(
		page.getByRole("status").filter({ hasText: "Pinned" }),
	).toBeVisible();
	await expect(
		page.getByRole("button", { name: "Save as new" }),
	).toBeDisabled();
});

// Kills an Open that leaves Explore to ask the API again for the lines its tile drew.
test("Open draws the pin in Explore from its tile", async ({ page }) => {
	await page.goto(PLOTS);
	const [title, drawn] = SEEDED[1];
	await expect(figure(page, title)).toHaveAccessibleName(drawn);
	const asked: string[] = [];
	page.on("request", (request) => {
		if (/\/v0\/console\/projects\/[^/]+\/perf/.test(request.url())) {
			asked.push(request.url());
		}
	});
	await page.getByRole("link", { name: `Open ${title} in Explore` }).click();
	await expect(
		page.getByRole("heading", { level: 1, name: title }),
	).toBeVisible();
	await expect(page.getByRole("figure")).toHaveAccessibleName("4 lines");
	expect(asked).toEqual([]);
});

// Kills a later batch that fails with no way to load it short of a reload, and
// one asked for again in a loop.
test("a batch that did not load says so and loads on Retry", async ({
	page,
	request,
}) => {
	const project = await pinnedProject(request, 8);
	const second = /\/v0\/console\/projects\/[^/]+\/plots\?(.*&)?page=2(&|$)/;
	let asked = 0;
	await page.route(second, (route) => {
		asked += 1;
		return route.fulfill({ status: 500, json: {} });
	});
	await page.goto(consolePath(project.slug, "plots"));
	await expect(figure(page, "Pin 1")).toHaveAccessibleName("2 lines over 4w");
	await tile(page, "Pin 8").scrollIntoViewIfNeeded();
	const failed = page.getByRole("alert").filter({ hasText: "did not load" });
	await expect(failed).toContainText("2 plots did not load", {
		timeout: 15_000,
	});
	await expect(figure(page, "Pin 8")).toHaveAccessibleName(
		"The plot did not load",
	);
	const tries = asked;
	await tile(page, "Pin 1").scrollIntoViewIfNeeded();
	await tile(page, "Pin 8").scrollIntoViewIfNeeded();
	await expect(failed).toBeVisible();
	expect(asked).toBe(tries);

	await page.unroute(second);
	await failed.getByRole("button", { name: "Retry" }).click();
	await expect(figure(page, "Pin 8")).toHaveAccessibleName("2 lines over 4w");
	await expect(figure(page, "Pin 7")).toHaveAccessibleName("2 lines over 4w");
	await expect(failed).toHaveCount(0);
});

// Kills editing controls drawn for a reader who cannot edit, and an Open a
// reader loses.
test.describe("a viewer", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	test("opens the pinned plots and changes none of them", async ({ page }) => {
		await page.goto(PLOTS);
		await expect(titles(page)).toHaveText(SEEDED.map(([title]) => title));
		await expect(page.getByText("Read only", { exact: true })).toBeVisible();
		await expect(
			page.getByRole("link", { name: /^Open .* in Explore$/ }),
		).toHaveCount(3);
		await expect(page.getByRole("button", { name: /^More for / })).toHaveCount(
			0,
		);
		await expect(page.getByRole("button", { name: "Reorder" })).toHaveCount(0);
		await expect(page.getByRole("link", { name: "New plot" })).toHaveCount(0);
	});
});

// Kills a contrast or a name missing in either theme.
test("Plots passes axe in both themes", async ({ page }) => {
	await axeInBothThemes(page, PLOTS, async () => {
		for (const [title, drawn] of SEEDED) {
			await expect(figure(page, title)).toHaveAccessibleName(drawn);
		}
	});
});

// Kills empty-state links that read only by their color, or a contrast or a
// name missing in either theme.
test("an empty Plots passes axe in both themes", async ({ page, request }) => {
	const project = await pinnedProject(request, 0);
	await axeInBothThemes(page, consolePath(project.slug, "plots"), async () => {
		await expect(
			page.getByRole("main").getByRole("link", { name: "report", exact: true }),
		).toBeVisible();
	});
});

test.describe("narrow", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a second column on a phone, and a Reorder mode that is not there or
	// whose targets are too small to touch.
	test("tiles stack in one column and Reorder mode moves them", async ({
		page,
		request,
	}) => {
		const project = await pinnedProject(request);
		await page.goto(consolePath(project.slug, "plots"));
		await expect(titles(page)).toHaveText(["Pin 1", "Pin 2", "Pin 3"]);
		const lefts = await Promise.all(
			["Pin 1", "Pin 2"].map(
				async (title) => (await tile(page, title).boundingBox())?.x,
			),
		);
		expect(lefts[0]).toBe(lefts[1]);

		await page.getByRole("button", { name: "Reorder" }).click();
		const up = page
			.getByRole("region", { name: "Reorder" })
			.getByRole("button", { name: "Move Pin 2 up" });
		expect((await up.boundingBox())?.height).toBeGreaterThanOrEqual(44);
		await up.click();
		await page.getByRole("button", { name: "Done" }).click();
		await expect(titles(page)).toHaveText(["Pin 2", "Pin 1", "Pin 3"]);
		await page.reload();
		await expect(titles(page)).toHaveText(["Pin 2", "Pin 1", "Pin 3"]);
	});
});
