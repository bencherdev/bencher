import type { Page } from "@playwright/test";
import {
	crumbs,
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	tabRow,
	test,
} from "./fixtures";
import ceilings from "./speed-ceilings.json" with { type: "json" };
import {
	type Cost,
	loadedAndIdle,
	mark,
	measure,
	observeLayoutShift,
} from "./speed";

const { hashbrown } = seed.projects;

test.use({ storageState: signedIn(seed.member), freezeClock: false });

const report = (name: string, cost: Cost) => {
	test
		.info()
		.annotations.push({ type: name, description: JSON.stringify(cost) });
	console.log(`${name}: ${JSON.stringify(cost)}`);
};

/** The page is up, its deferred work has started, and the network is quiet. */
const ready = async (page: Page) => {
	await expect(
		tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
	).toBeVisible();
	await loadedAndIdle(page);
	await settle(page);
};

// Kills JavaScript, document, and inline script growth, modules found only after
// their parent arrives, API requests that block the first paint or chain into a
// second round, and layout that shifts as data arrives.
test("a cold load holds the speed ceilings", async ({ page }) => {
	await observeLayoutShift(page);
	// An API slower than the first paint, so data never seen always arrives
	// after it and the shift it causes is the same on every run.
	await page.route(`${seed.api_url}/**`, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 500));
		await route.continue();
	});
	await page.goto(nextPath(hashbrown.slug, "reports"));
	await expect(
		crumbs(page).getByRole("link", { name: seed.organization.name }),
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
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills a returning reader's first paint that differs from what the app
// draws over it, and a reload that waits on the cache to ask the API.
test("a warm reload holds the speed ceilings", async ({ page }) => {
	await observeLayoutShift(page);
	await page.goto(nextPath(hashbrown.slug, "reports"));
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
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills a tab switch that refetches the shell or pulls in more than its page.
test("a tab switch holds the speed ceilings", async ({ page }) => {
	await page.goto(nextPath(hashbrown.slug, "reports"));
	await ready(page);

	const since = await mark(page);
	await tabRow(page).getByRole("link", { name: "Thresholds" }).click();
	await expect(
		page.getByRole("heading", { level: 1, name: "Thresholds" }),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("tab switch", cost);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceilings.tabSwitch.apiRequests);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceilings.tabSwitch.jsBytes);
});
