import type { APIRequestContext, Page } from "@playwright/test";
import {
	VERSION_KEY,
	crumb,
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	signedOut,
	tabRow,
	test,
} from "./fixtures";
import { listPlots } from "./api";
import { explore, twoLines } from "./explore";
import { publicPath, twoLinesDrawn, twoRecentLines } from "./public";
import ceilings from "./speed-ceilings.json" with { type: "json" };
import {
	type Cost,
	holdScripts,
	loadedAndIdle,
	mark,
	measure,
	observeLayoutShift,
} from "./speed";

const { hashbrown } = seed.projects;
// The shell alone: a project path no page draws, so no page data rides along.
const SHELL = nextPath(hashbrown.slug, "nowhere");
const RUNS = nextPath(hashbrown.slug, "runs");
// The project root, where bencher.dev lands a reader.
const PROJECT = nextPath(hashbrown.slug);
// A report's page, under no tab of its own.
const REPORTS = nextPath(hashbrown.slug, "reports");

test.use({ storageState: signedIn(seed.member) });

const report = (name: string, cost: Cost) => {
	test
		.info()
		.annotations.push({ type: name, description: JSON.stringify(cost) });
	console.log(`${name}: ${JSON.stringify(cost)}`);
};

/** The most the shell may move on a cold load at this page's width: the Alerts count arriving once. */
const shellCeiling = (page: Page) =>
	ceilings.shellShift[
		String(page.viewportSize()?.width) as keyof typeof ceilings.shellShift
	];

/** The page is up, its deferred work has started, and the network is quiet. */
const ready = async (page: Page) => {
	await expect(
		tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
	).toBeVisible();
	await loadedAndIdle(page);
	await settle(page);
};

// Kills JavaScript, stylesheet, document, and inline script growth, modules
// found only after their parent arrives, API requests that block the first paint
// or chain into a second round, and layout that shifts as data arrives.
test("a cold load holds the speed ceilings", async ({ page }) => {
	await observeLayoutShift(page);
	// An API slower than the first paint, so data never seen always arrives
	// after it and the shift it causes is the same on every run.
	await page.route(`${seed.api_url}/**`, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 500));
		await route.continue();
	});
	await page.goto(SHELL);
	await expect(
		crumb(page, `${seed.organization.name} / ${hashbrown.name}`),
	).toBeVisible();
	await ready(page);

	const cost = await measure(page, seed.api_url);
	report("cold load", cost);
	const ceiling = ceilings.coldLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.cssBytes).toBeLessThanOrEqual(ceiling.cssBytes);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills a returning reader's first paint that differs from what the app
// draws over it, and a reload that waits on the cache to ask the API.
test("a warm reload holds the speed ceilings", async ({ page }) => {
	await observeLayoutShift(page);
	await page.goto(SHELL);
	await ready(page);
	await settle(page, 1_500);

	await page.reload();
	await ready(page);

	const cost = await measure(page, seed.api_url);
	report("warm reload", cost);
	const ceiling = ceilings.warmReload;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.shellShift).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills a tab switch that refetches the shell, pulls in more than its page,
// or finds its code a round late.
test("a tab switch holds the speed ceilings", async ({ page }) => {
	await page.goto(SHELL);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await tabRow(page).getByRole("link", { name: "Settings" }).click();
	await expect(
		page.getByRole("heading", { level: 1, name: "General" }),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("tab switch", cost);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceilings.tabSwitch.apiRequests);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceilings.tabSwitch.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(
		ceilings.tabSwitch.scriptRounds,
	);
});

const runsReady = async (page: Page) => {
	await expect(
		page
			.getByRole("table", { name: "Runs, newest first" })
			.getByRole("link")
			.first(),
	).toBeVisible();
	await ready(page);
};

const runReports = (page: Page) =>
	page.getByRole("table", { name: "Reports in this run, by start" });

/** The newest run, through the API. */
const newestRun = async (request: APIRequestContext) => {
	const response = await request.get(
		`${seed.api_url}/v0/console/projects/${hashbrown.slug}/runs?per_page=1`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	const { runs } = (await response.json()) as { runs: { uuid: string }[] };
	return `${RUNS}/${runs[0]?.uuid}`;
};

/** The newest run's first report, through the API. */
const newestRunReport = async (request: APIRequestContext) => {
	const headers = { Authorization: `Bearer ${seed.member.token}` };
	const listed = await request.get(
		`${seed.api_url}/v0/console/projects/${hashbrown.slug}/runs?per_page=1`,
		{ headers },
	);
	const { runs } = (await listed.json()) as { runs: { uuid: string }[] };
	const run = await request.get(
		`${seed.api_url}/v0/console/projects/${hashbrown.slug}/runs/${runs[0]?.uuid}?per_page=1`,
		{ headers },
	);
	const { reports } = (await run.json()) as { reports: { uuid: string }[] };
	return reports[0]?.uuid;
};

// Kills a Runs page whose list waits on the shell's request or on its own
// code arriving late, JavaScript that grows, and rows that move as they arrive.
test("a cold load of Runs holds its speed ceilings", async ({ page }) => {
	await observeLayoutShift(page);
	await page.route(`${seed.api_url}/**`, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 500));
		await route.continue();
	});
	await page.goto(RUNS);
	await runsReady(page);

	const cost = await measure(page, seed.api_url);
	report("runs cold load", cost);
	const ceiling = ceilings.runsColdLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

test.describe("a returning reader", () => {
	test.use({
		storageState: signedIn(seed.member, {
			[VERSION_KEY]: JSON.stringify({ [hashbrown.slug]: 1 }),
		}),
	});

	// Kills a landing from bencher.dev that asks the API before it leaves the
	// root page or before the project page's first paint, and any growth in
	// what the project page it lands on loads.
	test("a cold landing from bencher.dev holds its speed ceilings", async ({
		page,
	}) => {
		await observeLayoutShift(page);
		let apiRequestsBeforeLanding = 0;
		let landed = false;
		page.on("request", (request) => {
			if (request.url() === new URL(PROJECT, seed.console_url).href) {
				landed = true;
			} else if (!landed && request.url().startsWith(seed.api_url)) {
				apiRequestsBeforeLanding += 1;
			}
		});
		await page.route(`${seed.api_url}/**`, async (route) => {
			await new Promise((resolve) => setTimeout(resolve, 500));
			await route.continue();
		});
		await page.goto("/");
		await expect(page).toHaveURL(PROJECT);
		await ready(page);

		const cost = {
			...(await measure(page, seed.api_url)),
			apiRequestsBeforeLanding,
		};
		report("landing cold load", cost);
		const ceiling = ceilings.landingColdLoad;
		expect(cost.apiRequestsBeforeLanding).toBeLessThanOrEqual(
			ceiling.apiRequestsBeforeLanding,
		);
		expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
			ceiling.apiAnswersBeforePaint,
		);
		expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
		expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
		expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
		expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
		expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
		expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
		expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
		expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
		expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
			ceiling.inlineScriptBytes,
		);
		expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
	});
});

// Kills a switch into Runs that refetches the shell, asks for the list in
// more than one request, pulls in more than its page, or finds its code a
// round late.
test("a tab switch into Runs holds its speed ceilings", async ({ page }) => {
	await page.goto(SHELL);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await tabRow(page).getByRole("link", { name: "Runs" }).click();
	await runsReady(page);

	const cost = await measure(page, seed.api_url, since);
	report("runs tab switch", cost);
	const ceiling = ceilings.runsTabSwitch;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a move to a run that refetches the shell or the list, asks for the
// run in more than one request, pulls in more than its page, or finds its code
// a round late.
test("a move from Runs to a run holds its speed ceilings", async ({ page }) => {
	await page.goto(RUNS);
	await runsReady(page);

	await holdScripts(page);
	const since = await mark(page);
	await page
		.getByRole("table", { name: "Runs, newest first" })
		.getByRole("link")
		.first()
		.click();
	await expect(runReports(page).getByRole("link").first()).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("run move", cost);
	const ceiling = ceilings.runMove;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a Feedback dialog whose code loads in more than one round, asks the
// API for anything, or grows.
test("opening Feedback holds its speed ceilings", async ({ page }) => {
	await page.goto(RUNS);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await page.getByRole("button", { name: "Feedback", exact: true }).click();
	await expect(
		page.getByRole("dialog", { name: "Send feedback" }),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("feedback open", cost);
	const ceiling = ceilings.feedbackOpen;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a switcher whose code loads in more than one round, whose projects
// take more than one request, or that grows.
test("opening the switcher holds its speed ceilings", async ({ page }) => {
	await page.goto(RUNS);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await crumb(page).click();
	await expect(
		page
			.getByRole("dialog", { name: "Switch project" })
			.getByRole("option", { name: new RegExp(`^${hashbrown.name}`) }),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("switcher open", cost);
	const ceiling = ceilings.switcherOpen;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a move to a report that refetches the shell or the run, pulls in more
// than its page, finds its code a round late, or defers more than its rows'
// links.
test("a move from a run to a report holds its speed ceilings", async ({
	page,
	request,
}) => {
	await page.goto(await newestRun(request));
	await expect(runReports(page).getByRole("link").first()).toBeVisible();
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await runReports(page).getByRole("link").first().click();
	await expect(
		page
			.getByRole("table", { name: /^Lines in report / })
			.getByRole("checkbox")
			.first(),
	).toBeVisible();
	// The rows' own links arrive once the page goes idle, as deferred code.
	await expect(
		page.getByRole("link", { name: /^Open in Explore: / }).first(),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("report move", cost);
	const ceiling = ceilings.reportMove;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a step to the run before on the branch that refetches the shell or the
// run it left, asks for the run in more than one request, or loads code.
test("a move from a run to the run before it holds its speed ceilings", async ({
	page,
	request,
}) => {
	await page.goto(await newestRun(request));
	await expect(runReports(page).getByRole("link").first()).toBeVisible();
	await ready(page);
	const heading = await page.getByRole("heading", { level: 1 }).textContent();

	await holdScripts(page);
	const since = await mark(page);
	await page.getByRole("link", { name: /^Previous / }).click();
	await expect(page.getByRole("heading", { level: 1 })).not.toHaveText(
		heading ?? "",
	);
	await expect(runReports(page).getByRole("link").first()).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("run to run move", cost);
	const ceiling = ceilings.runToRunMove;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a move from a report back to its run that refetches the shell or the
// report, asks for the run in more than one request, or finds its code a round late.
test("a move from a report to its run holds its speed ceilings", async ({
	page,
	request,
}) => {
	await page.goto(`${REPORTS}/${await newestRunReport(request)}`);
	const crumb = page.getByRole("link", {
		name: `main ${seed.last_main_hash}`,
		exact: true,
	});
	await expect(crumb).toBeVisible();
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await crumb.click();
	await expect(runReports(page).getByRole("link").first()).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("report to run move", cost);
	const ceiling = ceilings.reportToRunMove;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

/** Load `path` cold, the API slower than the first paint, and measure it once `shown` resolves. */
const coldLoad = async (
	page: Page,
	path: string,
	shown: () => Promise<void>,
	delay: (url: string) => number = () => 500,
): Promise<Cost> => {
	await observeLayoutShift(page);
	await page.route(`${seed.api_url}/**`, async (route) => {
		await new Promise((resolve) =>
			setTimeout(resolve, delay(route.request().url())),
		);
		await route.continue();
	});
	await page.goto(path);
	await shown();
	await ready(page);
	return measure(page, seed.api_url);
};

// Kills a General section that asks the API for what the shell's request
// already holds, and one that shifts as the project arrives.
test("a cold load of General holds the speed ceilings", async ({ page }) => {
	const cost = await coldLoad(page, nextPath(hashbrown.slug, "settings"), () =>
		expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			hashbrown.name,
		),
	);
	report("settings cold load", cost);
	const ceiling = ceilings.settingsColdLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills key lists that wait on the shell's request, a third request for the
// other status, and a table that shifts as the keys arrive.
test("a cold load of Keys holds the speed ceilings", async ({ page }) => {
	const cost = await coldLoad(page, nextPath(hashbrown.slug, "keys"), () =>
		expect(page.getByRole("table", { name: "Active keys" })).toBeVisible(),
	);
	report("keys cold load", cost);
	const ceiling = ceilings.keysColdLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Measures each page's cold load on a narrow screen: what the page moves as its
// data arrives, and what the shell moves when the Alerts count arrives.
for (const [width, viewport] of [
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`narrow cold loads at ${width}`, () => {
		test.use({ viewport });
		for (const [name, path, shown, ceiling] of [
			[
				"the shell",
				SHELL,
				(page: Page) =>
					expect(
						tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
					).toBeVisible(),
				ceilings.coldLoad,
			],
			["Runs", RUNS, runsReady, ceilings.runsColdLoad],
			[
				"General",
				nextPath(hashbrown.slug, "settings"),
				(page: Page) =>
					expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
						hashbrown.name,
					),
				ceilings.settingsColdLoad,
			],
			[
				"Keys",
				nextPath(hashbrown.slug, "keys"),
				(page: Page) =>
					expect(
						page.getByRole("table", { name: "Active keys" }),
					).toBeVisible(),
				ceilings.keysColdLoad,
			],
		] as const) {
			test(`a cold load of ${name} moves nothing at ${width}`, async ({
				page,
			}) => {
				const cost = await coldLoad(page, path, () => shown(page));
				report(`${name} cold load ${width}`, cost);
				expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
				expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
			});
		}
	});
}

// Kills a section switch that loads more code or refetches the shell.
test("moving from General to Keys holds the speed ceilings", async ({
	page,
}) => {
	await page.goto(nextPath(hashbrown.slug, "settings"));
	await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
		hashbrown.name,
	);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await page
		.getByRole("navigation", { name: "Settings", exact: true })
		.getByRole("link", { name: "Keys" })
		.click();
	await expect(page.getByRole("table", { name: "Active keys" })).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("settings section switch", cost);
	expect(cost.apiRequests).toBeLessThanOrEqual(
		ceilings.settingsSectionSwitch.apiRequests,
	);
	expect(cost.apiRounds).toBeLessThanOrEqual(
		ceilings.settingsSectionSwitch.apiRounds,
	);
	expect(cost.jsBytes).toBeLessThanOrEqual(
		ceilings.settingsSectionSwitch.jsBytes,
	);
	expect(cost.scriptRounds).toBeLessThanOrEqual(
		ceilings.settingsSectionSwitch.scriptRounds,
	);
});

/** The plot holds the seed's newest `main` report, whose alert only a window reaching it draws. */
const alertDrawn = (page: Page) =>
	expect(
		page.getByRole("figure").getByText("1 alert", { exact: true }),
	).toBeVisible();

const twoLinesShown = async (page: Page) => {
	await expect(page.getByRole("figure")).toHaveAccessibleName("2 lines");
	await alertDrawn(page);
};

/** The seed's pin. */
const PINNED = "blake3 input_bytes=65536 threads=1, Latency on main";

/** The seed's pinned tile, its alert drawn: its window reaches the newest `main` report. */
const alertingTileShown = async (page: Page) => {
	const tile = page.getByRole("region", { name: PINNED });
	await expect(tile.getByRole("figure")).toHaveAccessibleName(
		"2 lines over 4w",
	);
	await expect(tile.getByRole("img", { name: "has an alert" })).toBeVisible();
};

// The cold loads of a report, at each width the layout changes at.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`at ${width}`, () => {
		if (viewport) {
			test.use({ viewport });
		}

		// Kills a report whose lines wait on the shell's request or on its own
		// code arriving late, a first batch asked for twice, the full plot's code
		// loaded before a row expands, a page stylesheet of its own, and a page
		// that shifts as the report arrives.
		test(`a cold load of a report holds its speed ceilings at ${width}`, async ({
			page,
			request,
		}) => {
			const response = await request.get(
				`${seed.api_url}/v0/projects/${hashbrown.slug}/reports?branch=main&testbed=ubuntu-latest&per_page=1`,
				{ headers: { Authorization: `Bearer ${seed.member.token}` } },
			);
			const [newest] = (await response.json()) as { uuid: string }[];
			const cost = await coldLoad(page, `${REPORTS}/${newest?.uuid}`, () =>
				expect(
					page
						.getByRole("table", { name: /^Lines in report / })
						.getByRole("checkbox")
						.first(),
				).toBeVisible(),
			);
			report(`report cold load ${width}`, cost);
			const ceiling = ceilings.reportColdLoad;
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
			expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
		});

		// Kills a run whose reports wait on the shell's request or on its own code
		// arriving late, a first batch asked for twice, a page stylesheet of its
		// own, and a page that shifts as the run arrives.
		test(`a cold load of a run holds its speed ceilings at ${width}`, async ({
			page,
			request,
		}) => {
			const cost = await coldLoad(page, await newestRun(request), () =>
				expect(runReports(page).getByRole("link").first()).toBeVisible(),
			);
			report(`run cold load ${width}`, cost);
			const ceiling = ceilings.runColdLoad;
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
			expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
		});

		test.describe("as a viewer", () => {
			test.use({ storageState: signedIn(seed.outsider) });

			for (const [order, delay] of [
				// The order of every move to a report within the console.
				[
					"its bootstrap answering first",
					(url: string) =>
						/\/v0\/console\/projects\/[^/]+\/reports\//.test(url) ? 1_000 : 500,
				],
				[
					"its report answering first",
					(url: string) =>
						/\/v0\/console\/projects\/[^/]+\/reports\//.test(url) ? 500 : 1_000,
				],
			] as const) {
				// Kills a viewer's Read only pill that the title and its alerts push
				// along, one that takes its width from the title, cutting it, and
				// one that moves the title's alerts as it arrives.
				test(`a viewer's cold load of a report, ${order}, holds its speed ceilings at ${width}`, async ({
					page,
					request,
				}) => {
					const response = await request.get(
						`${seed.api_url}/v0/projects/${hashbrown.slug}/reports?branch=main&testbed=ubuntu-latest&per_page=1`,
						{ headers: { Authorization: `Bearer ${seed.member.token}` } },
					);
					const [newest] = (await response.json()) as { uuid: string }[];
					const cost = await coldLoad(
						page,
						`${REPORTS}/${newest?.uuid}`,
						async () => {
							await expect(
								page.getByText("Read only", { exact: true }),
							).toBeVisible();
							await expect(
								page.getByRole("table", { name: /^Lines in report / }),
							).toBeVisible();
						},
						delay,
					);
					report(`viewer report cold load ${order} ${width}`, cost);
					const ceiling = ceilings.reportColdLoad;
					expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
					expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
					expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
					expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
					expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
					expect(cost.deferredJsBytes).toBeLessThanOrEqual(
						ceiling.deferredJsBytes,
					);
					const title = page.getByRole("heading", { level: 1 });
					await expect(title).toHaveText("main · ubuntu-latest");
					expect(
						await title.evaluate(
							(heading) => heading.scrollWidth <= heading.clientWidth,
						),
					).toBe(true);
				});
			}
		});
	});
}

// The cold loads that draw Explore, at each width the layout changes at.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`at ${width}`, () => {
		if (viewport) {
			test.use({ viewport });
		}

		// Kills an Explore link whose plot query waits on the shell's request, a
		// second data request, code found late, a page stylesheet of its own, and
		// a page that shifts as the answer arrives.
		test(`a cold Explore link holds its speed ceilings at ${width}`, async ({
			page,
			request,
		}) => {
			const link = explore(hashbrown.slug, (await twoLines(request)).query);
			const cost = await coldLoad(page, link, () => twoLinesShown(page));
			report(`explore cold load ${width}`, cost);
			const ceiling = ceilings.exploreColdLoad;
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
			const stylesheets = await page.evaluate(
				() =>
					performance
						.getEntriesByType("resource")
						.filter((entry) => new URL(entry.name).pathname.endsWith(".css"))
						.length,
			);
			expect(stylesheets).toBe(1);
		});

		// Kills tiles that ask for their lines one request each or after the
		// shell's request, code found late or after the load, a page stylesheet
		// of its own, and tiles that shift as their lines arrive.
		test(`a cold Plots load holds its speed ceilings at ${width}`, async ({
			page,
		}) => {
			const cost = await coldLoad(page, nextPath(hashbrown.slug, "plots"), () =>
				alertingTileShown(page),
			);
			report(`plots cold load ${width}`, cost);
			const ceiling = ceilings.plotsColdLoad;
			// The plot's code is named in the document, never left for after the load.
			expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
			const stylesheets = await page.evaluate(
				() =>
					performance
						.getEntriesByType("resource")
						.filter((entry) => new URL(entry.name).pathname.endsWith(".css"))
						.length,
			);
			expect(stylesheets).toBe(1);
		});

		// Kills starting points asked for one after another or after the shell's
		// request, and lists that shift as they arrive.
		test(`a cold blank Explore holds its speed ceilings at ${width}`, async ({
			page,
		}) => {
			const cost = await coldLoad(page, nextPath(hashbrown.slug), () =>
				expect(
					page
						.getByRole("region", { name: "Alerting now" })
						.getByRole("link", { name: /^blake3 / }),
				).toBeVisible(),
			);
			report(`blank explore cold load ${width}`, cost);
			const ceiling = ceilings.blankExploreColdLoad;
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
		});
	});
}

// A link that names only a pin, at each width the query panel changes at.
for (const viewport of [
	{ width: 390, height: 844 },
	{ width: 767, height: 1024 },
	{ width: 768, height: 1024 },
	{ width: 1024, height: 768 },
	{ width: 1280, height: 720 },
]) {
	test.describe(`a bare pin at ${viewport.width}`, () => {
		test.use({ viewport });

		// Kills a pin link that paints its header, boxes, or Clear before the
		// pin's query arrives, which then moves the page.
		test(`a cold link to a pin alone moves nothing at ${viewport.width}`, async ({
			page,
			request,
		}) => {
			const pin = (await listPlots(request, hashbrown.slug)).find(
				({ title }) => title === PINNED,
			);
			expect(pin).toBeDefined();
			const cost = await coldLoad(
				page,
				`${nextPath(hashbrown.slug)}?plot=${pin?.uuid}`,
				async () => {
					await expect(
						page.getByRole("heading", { level: 1, name: PINNED }),
					).toBeVisible();
					await twoLinesShown(page);
				},
			);
			report(`bare pin cold load ${viewport.width}`, cost);
			expect(cost.cls).toBeLessThanOrEqual(ceilings.exploreColdLoad.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
		});
	});
}

// Kills a link whose plot code is found only once the page's own code runs, a
// round later, blank Explore loading the plot it does not draw, and a reader
// reaching for a benchmark who waits on the plot's code after the pick.
test("an Explore link names its plot's code in its document; blank Explore loads it only once a benchmark is near", async ({
	newContext,
	page,
	request,
}) => {
	const scripts = (target: Page) =>
		target.evaluate(() =>
			performance
				.getEntriesByType("resource")
				.map((entry) => new URL(entry.name))
				.filter(
					(url) =>
						url.origin === location.origin && url.pathname.endsWith(".js"),
				)
				.map((url) => url.pathname),
		);
	await page.goto(nextPath(hashbrown.slug));
	await expect(
		page
			.getByRole("region", { name: "Alerting now" })
			.getByRole("link", { name: /^blake3 / }),
	).toBeVisible();
	await ready(page);
	const blank = new Set(await scripts(page));

	const context = await newContext({
		storageState: signedIn(seed.member),
	});
	const linked = await context.newPage();
	const link = explore(hashbrown.slug, (await twoLines(request)).query);
	await linked.goto(link);
	await twoLinesShown(linked);
	await ready(linked);
	const served = await (await linked.request.get(linked.url())).text();
	const named = new Set(
		[...served.matchAll(/<link\b[^>]*\bhref="([^"]+\.js)"/g)].map(
			([, href]) => new URL(href ?? "", linked.url()).pathname,
		),
	);
	const plot = (await scripts(linked)).filter((path) => !blank.has(path));
	expect(plot.length).toBeGreaterThan(0);
	expect(plot.filter((path) => !named.has(path))).toEqual([]);
	await context.close();

	await page
		.getByRole("group", { name: "Benchmarks" })
		.getByRole("button", { name: "Add benchmark" })
		.click();
	await expect
		.poll(async () => {
			const loaded = new Set(await scripts(page));
			return plot.filter((path) => !loaded.has(path));
		})
		.toEqual([]);
});

// Kills a box edit that asks the API more than once or loads more code.
test("an Explore box edit makes one request", async ({ page, request }) => {
	await page.goto(explore(hashbrown.slug, (await twoLines(request)).query));
	await twoLinesShown(page);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await page
		.getByRole("group", { name: "Parameters" })
		.getByRole("button", { name: "Remove set input_bytes=65536 threads=1" })
		.click();
	await expect(page.getByRole("figure")).toHaveAccessibleName("8 lines");
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("explore box edit", cost);
	expect(cost.apiRequests).toBeLessThanOrEqual(
		ceilings.exploreBoxEdit.apiRequests,
	);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceilings.exploreBoxEdit.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceilings.exploreBoxEdit.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(
		ceilings.exploreBoxEdit.scriptRounds,
	);
});

// Kills a move to a plot whose code waits on the page's own, or whose own
// imports are found only once it arrives: a lazy import that lost its
// preload list, or a preload rule that drops what the plot needs.
test("a move from blank Explore to an Explore link holds its speed ceilings", async ({
	page,
}) => {
	const alerting = page
		.getByRole("region", { name: "Alerting now" })
		.getByRole("link", { name: /^blake3 / })
		.first();
	await page.goto(nextPath(hashbrown.slug));
	await expect(alerting).toBeVisible();
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await alerting.click();
	await expect(page.getByRole("figure")).toHaveAccessibleName(/^\d+ lines?$/);
	await alertDrawn(page);
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("explore link move", cost);
	const ceiling = ceilings.exploreLinkMove;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a key toggle that refetches, rebuilds the plot, or does more than one
// 120 Hz frame of work through the link it writes.
test("an Explore key toggle fits in a frame", async ({ page, request }) => {
	await page.goto(explore(hashbrown.slug, (await twoLines(request)).query));
	await twoLinesShown(page);
	await ready(page);

	const since = await mark(page);
	const times = await page.evaluate(async () => {
		const toggle = /^blake3 simd=sse4\.2.*: (hide|show) its line$/;
		const button = () =>
			[...document.querySelectorAll("button")].find((each) =>
				toggle.test(each.getAttribute("aria-label") ?? ""),
			);
		// A message is a task of its own that no timer clamp delays, so a wait
		// for the redraw measures the work and not the browser's timers.
		const nextTask = () =>
			new Promise<void>((resolve) => {
				const channel = new MessageChannel();
				channel.port1.onmessage = () => resolve();
				channel.port2.postMessage(null);
			});
		const nextFrame = () =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
			);
		const canvas = document.querySelector("figure canvas");
		const spent: number[] = [];
		for (let round = 0; round < 9; round++) {
			const before = button()?.getAttribute("aria-label");
			const start = performance.now();
			button()?.click();
			while (button()?.getAttribute("aria-label") === before) {
				await nextTask();
			}
			spent.push(performance.now() - start);
			await nextFrame();
		}
		return {
			times: spent.sort((a, b) => a - b),
			kept: canvas?.isConnected === true,
		};
	});
	const median =
		times.times[Math.floor(times.times.length / 2)] ?? Number.POSITIVE_INFINITY;
	report("explore key toggle", { median, ...times } as unknown as Cost);
	// The plot redraws in place rather than building itself again.
	expect(times.kept).toBe(true);
	expect(median).toBeLessThanOrEqual(ceilings.exploreKeyToggle.ms);
	const cost = await measure(page, seed.api_url, since);
	expect(cost.apiRequests).toBe(0);
});

const THRESHOLDS = nextPath(hashbrown.slug, "thresholds");

// Kills a switch into Thresholds that refetches the shell, asks for the list in
// more than one request, pulls in more than its page, or finds its code a
// round late.
test("a tab switch into Thresholds holds its speed ceilings", async ({
	page,
}) => {
	await page.goto(SHELL);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await tabRow(page).getByRole("link", { name: "Thresholds" }).click();
	await expect(
		page.getByRole("table", { name: /^Thresholds, with the alerts/ }),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("thresholds tab switch", cost);
	const ceiling = ceilings.thresholdsTabSwitch;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a switch into Plots that refetches the shell, asks for its tiles'
// lines in more than one round, or finds the plot's code a round or two late:
// a lazy import that lost its preload list.
test("a tab switch into Plots holds its speed ceilings", async ({ page }) => {
	await page.goto(SHELL);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await tabRow(page).getByRole("link", { name: "Plots" }).click();
	await alertingTileShown(page);
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("plots tab switch", cost);
	const ceiling = ceilings.plotsTabSwitch;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// The cold loads of Thresholds and of a threshold, at each width the layout changes at.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`thresholds at ${width}`, () => {
		if (viewport) {
			test.use({ viewport });
		}

		const holds = (
			page: Page,
			cost: Cost,
			ceiling: typeof ceilings.thresholdsColdLoad,
		) => {
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
			expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
		};

		// Kills a list that waits on the shell's request or on its own code
		// arriving late, a page stylesheet of its own, and rows or a count that
		// move the page as they arrive.
		test(`a cold load of Thresholds holds its speed ceilings at ${width}`, async ({
			page,
		}) => {
			const cost = await coldLoad(page, THRESHOLDS, () =>
				expect(
					page
						.getByRole("table", { name: /^Thresholds, with the alerts/ })
						.getByRole("link")
						.first(),
				).toBeVisible(),
			);
			report(`thresholds cold load ${width}`, cost);
			holds(page, cost, ceilings.thresholdsColdLoad);
		});

		// Kills a threshold whose alerts wait on the threshold's own request, a
		// row's full plot loaded before a row expands, and cards or rows that
		// move the page as they arrive.
		test(`a cold load of a threshold holds its speed ceilings at ${width}`, async ({
			page,
			request,
		}) => {
			const response = await request.get(
				`${seed.api_url}/v0/console/projects/${hashbrown.slug}/thresholds`,
				{ headers: { Authorization: `Bearer ${seed.member.token}` } },
			);
			const {
				thresholds: [threshold],
			} = (await response.json()) as { thresholds: { uuid: string }[] };
			const cost = await coldLoad(
				page,
				`${THRESHOLDS}/${threshold?.uuid}`,
				// The alerts sit below the cards, out of view on a phone, so their count says they arrived.
				() =>
					expect(
						page.getByText(/^[1-9]\d* active · \d+ dismissed$/),
					).toBeVisible(),
			);
			report(`threshold cold load ${width}`, cost);
			holds(page, cost, ceilings.thresholdColdLoad);
		});
	});
}

// The cold loads of a dimension list and a dimension's page, at each width
// the layout changes at.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`dimensions at ${width}`, () => {
		if (viewport) {
			test.use({ viewport });
		}

		// Kills a list or a page whose requests wait on the shell's or chain
		// into a second round, code found late, a stylesheet of its own, and a
		// page that shifts as its data arrives.
		for (const [name, path, shown, ceiling] of [
			[
				"branches",
				"branches",
				(page: Page) =>
					page
						.getByRole("table", { name: "Active branches" })
						.getByRole("link", { name: "main", exact: true }),
				ceilings.branchesColdLoad,
			],
			[
				"branch",
				"branches/main",
				(page: Page) =>
					page.getByRole("link", {
						name: /^ubuntu-latest, threshold for Latency on main, ubuntu-latest/,
					}),
				ceilings.branchColdLoad,
			],
		] as const) {
			test(`a cold load of ${name} holds its speed ceilings at ${width}`, async ({
				page,
			}) => {
				const cost = await coldLoad(
					page,
					nextPath(hashbrown.slug, path),
					async () => {
						await expect(shown(page)).toBeVisible();
					},
				);
				report(`${name} cold load ${width}`, cost);
				expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
					ceiling.apiAnswersBeforePaint,
				);
				expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
				expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
				expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
				expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
				expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
				expect(cost.deferredJsBytes).toBeLessThanOrEqual(
					ceiling.deferredJsBytes,
				);
				expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
				expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
				expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
			});
		}
	});
}

// The cold loads of Alerts, at each width the layout changes at.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`Alerts at ${width}`, () => {
		if (viewport) {
			test.use({ viewport });
		}

		// Kills alerts that wait on the shell's request or on their own code
		// arriving late, a first batch asked for twice, a filter name asked for
		// with no filter set, the full plot's code loaded before a row expands, a
		// page stylesheet of its own, and a page that shifts as its alerts arrive.
		test(`a cold load of Alerts holds its speed ceilings at ${width}`, async ({
			page,
		}) => {
			const cost = await coldLoad(
				page,
				nextPath(hashbrown.slug, "alerts"),
				() =>
					expect(
						page
							.getByRole("table", { name: /^Alerts/ })
							.getByRole("checkbox")
							.last(),
					).toBeVisible(),
			);
			report(`alerts cold load ${width}`, cost);
			const ceiling = ceilings.alertsColdLoad;
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
			expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
		});
	});
}

/** The seed's one active alert, as the API lists it. */
const activeAlert = async (request: APIRequestContext) => {
	const response = await request.get(
		`${seed.api_url}/v0/projects/${hashbrown.slug}/alerts?status=active`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	const [alert] = (await response.json()) as { uuid: string }[];
	if (!alert) {
		throw new Error("The seed has no active alert");
	}
	return alert.uuid;
};
const ALERT_NAME = "blake3 input_bytes=65536 simd=avx2 threads=1 Latency";

/** The alert's page is drawn, its plot pinned on the alert. */
const alertShown = (page: Page) =>
	expect(page.getByRole("figure").getByRole("status")).toHaveAttribute(
		"aria-live",
		"polite",
	);

const expectAlertColdLoad = (page: Page, cost: Cost) => {
	const ceiling = ceilings.alertColdLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.shellShift).toBeLessThanOrEqual(shellCeiling(page));
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
};

// The cold loads of an alert's page, at each width the layout changes at.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`an alert at ${width}`, () => {
		if (viewport) {
			test.use({ viewport });
		}

		// Kills an alert that waits on the shell's request or on its own code
		// or its plot's arriving late, an alert asked for twice, a page
		// stylesheet of its own, and a page that shifts as the alert arrives.
		test(`a cold load of an alert holds its speed ceilings at ${width}`, async ({
			page,
			request,
		}) => {
			const alert = await activeAlert(request);
			const cost = await coldLoad(
				page,
				nextPath(hashbrown.slug, `alerts/${alert}`),
				async () => {
					await alertShown(page);
					await expect(
						page.getByRole("button", {
							name: `Dismiss the alert on ${ALERT_NAME}`,
						}),
					).toBeVisible();
				},
			);
			report(`alert cold load ${width}`, cost);
			expectAlertColdLoad(page, cost);
		});

		test.describe("as a viewer", () => {
			test.use({ storageState: signedIn(seed.outsider) });

			for (const [order, delay] of [
				[
					"its bootstrap answering first",
					(url: string) =>
						/\/v0\/console\/projects\/[^/]+\/alerts\//.test(url) ? 1_000 : 500,
				],
				[
					"its alert answering first",
					(url: string) =>
						/\/v0\/console\/projects\/[^/]+\/alerts\//.test(url) ? 500 : 1_000,
				],
			] as const) {
				// Kills a viewer's page that draws before it knows the reader cannot
				// change the alert, and so moves when that arrives.
				test(`a viewer's cold load of an alert, ${order}, holds its speed ceilings at ${width}`, async ({
					page,
					request,
				}) => {
					const alert = await activeAlert(request);
					const cost = await coldLoad(
						page,
						nextPath(hashbrown.slug, `alerts/${alert}`),
						async () => {
							await expect(
								page.getByText("Read only", { exact: true }),
							).toBeVisible();
							await alertShown(page);
						},
						delay,
					);
					report(`viewer alert cold load ${order} ${width}`, cost);
					expectAlertColdLoad(page, cost);
				});
			}
		});
	});
}

// Kills a move to an alert that refetches the shell or the list, asks for
// its alert more than once, finds its plot's code a round late, or pulls in
// more than its page.
test("a move from Alerts to an alert holds its speed ceilings", async ({
	page,
}) => {
	await page.goto(nextPath(hashbrown.slug, "alerts"));
	await expect(
		page.getByRole("link", { name: /^Open in Explore: / }).first(),
	).toBeVisible();
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await page
		.getByRole("link", { name: `${ALERT_NAME}, open the alert` })
		.click();
	await alertShown(page);
	await expect(
		page.getByRole("link", { name: `Open in Explore: ${ALERT_NAME}` }),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("alert move", cost);
	const ceiling = ceilings.alertMove;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a status switch that asks for more than its first batch, chains a
// second round, or loads more code, before or after its list's idle load.
test("a status switch on Alerts asks once", async ({ page }) => {
	await page.goto(nextPath(hashbrown.slug, "alerts"));
	await expect(
		page
			.getByRole("table", { name: /^Alerts/ })
			.getByRole("checkbox")
			.last(),
	).toBeVisible();
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await page.getByRole("radio", { name: "Dismissed" }).check();
	await expect(
		page
			.getByRole("table", { name: /^Alerts/ })
			.getByRole("button", { name: /^Reactivate the alert on sha256/ }),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("alerts status switch", cost);
	const ceiling = ceilings.alertsViewSwitch;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// Kills a Clear that asks for more than the list's first batch, chains a
// second round, or loads more code.
test("a Clear on Runs asks once", async ({ page }) => {
	await page.goto(`${RUNS}?branch=main`);
	await runsReady(page);
	await ready(page);

	await holdScripts(page);
	const since = await mark(page);
	await page.getByRole("button", { name: "Clear filters" }).click();
	await expect(page.getByRole("heading", { level: 1 })).toHaveText("15 runs");
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("runs clear", cost);
	const ceiling = ceilings.runsClear;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.scriptRounds).toBeLessThanOrEqual(ceiling.scriptRounds);
});

// The public plot: a crawled and shared page that paints from its document and
// one stylesheet, then asks the API once.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`the public plot at ${width}`, () => {
		// Its lines reported near the real time (see `twoRecentLines`).
		test.use({
			storageState: signedOut,
			freezeClock: false,
			...(viewport ? { viewport } : {}),
		});

		// Kills a public page that waits on the API or on code to paint, asks
		// the API twice or in two rounds, finds code late, links a second
		// stylesheet or the styles of the console's other pages, or shifts as
		// the plot arrives.
		test(`a cold public plot holds its speed ceilings at ${width}`, async ({
			page,
			request,
		}) => {
			const { project, search } = await twoRecentLines(request);
			await observeLayoutShift(page);
			await page.route(`${seed.api_url}/**`, async (route) => {
				await new Promise((resolve) => setTimeout(resolve, 500));
				await route.continue();
			});
			await page.goto(publicPath(project.slug, search));
			await twoLinesDrawn(page);
			await loadedAndIdle(page);
			await settle(page);
			const cost = await measure(page, seed.api_url);
			// What the first paint waited on: the stylesheet, and nothing else.
			const blocking = await page.evaluate(() =>
				(
					performance.getEntriesByType(
						"resource",
					) as (PerformanceResourceTiming & {
						renderBlockingStatus?: string;
					})[]
				)
					.filter((entry) => entry.renderBlockingStatus === "blocking")
					.map((entry) => entry.name),
			);
			report(`public plot cold load ${width}`, {
				...cost,
				blocking: blocking.length,
			} as Cost);
			const ceiling = ceilings.publicColdLoad;
			expect(blocking).toHaveLength(1);
			expect(blocking[0]).toMatch(/\.css$/);
			expect(cost.apiAnswersBeforePaint).toBe(0);
			expect(cost.apiRequests).toBe(1);
			expect(cost.apiRounds).toBe(1);
			expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.shellShift).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
			expect(cost.cssBytes).toBeLessThanOrEqual(ceiling.cssBytes);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBe(0);
		});
	});
}

// Kills code moved onto the classic console's pages, which every one of them
// would then load: a shared file the new console's groups fold more into.
test("a cold load of a classic perf page holds its byte ceilings", async ({
	page,
}) => {
	await page.goto(`/console/projects/${seed.projects.version_zero.slug}/perf`);
	await loadedAndIdle(page);
	await settle(page, 600);

	const cost = await measure(page, seed.api_url);
	report("classic perf cold load", cost);
	const ceiling = ceilings.classicPerfColdLoad;
	// An island imports its code once it runs, before or after the load event.
	expect(cost.jsBytes + cost.deferredJsBytes).toBeLessThanOrEqual(
		ceiling.jsBytes,
	);
	expect(cost.cssBytes).toBeLessThanOrEqual(ceiling.cssBytes);
});
