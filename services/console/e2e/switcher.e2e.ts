import type { Page } from "@playwright/test";
import {
	crumb,
	expect,
	consolePath,
	seed,
	settle,
	signedIn,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown, iterations, version_zero, empty } = seed.projects;
const RUNS = consolePath(hashbrown.slug, "runs");
const BOOTSTRAP = `${seed.api_url}/v0/console/users/${seed.member.user.uuid}`;

test.use({ storageState: signedIn(seed.member) });

const dialog = (page: Page) =>
	page.getByRole("dialog", { name: "Switch project" });
const search = (page: Page) =>
	dialog(page).getByRole("combobox", { name: "Search projects" });
const option = (page: Page, name: string) =>
	dialog(page).getByRole("option", { name: new RegExp(`^${name}`) });

/** Runs of Hashbrown, drawn and quiet. */
const arrive = async (page: Page) => {
	await page.goto(RUNS);
	await expect(
		tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
	).toBeVisible();
	await settle(page);
};

// Kills a switcher that does not list the reader's projects as the bootstrap
// has them, and a pick that loads a new document rather than moving in the app.
test("the crumb opens the switcher on the reader's projects, and a pick moves in the app", async ({
	page,
}) => {
	await arrive(page);
	const documents: string[] = [];
	page.on("request", (request) => {
		if (request.resourceType() === "document") {
			documents.push(request.url());
		}
	});

	await crumb(page, `${seed.organization.name} / ${hashbrown.name}`).click();

	await expect(option(page, hashbrown.name)).toContainText("Current");
	await search(page).fill(empty.name);
	await expect(option(page, empty.name)).toContainText("No runs");
	await search(page).fill(iterations.name);
	await option(page, iterations.name).click();

	await expect(page).toHaveURL(consolePath(iterations.slug));
	await expect(dialog(page)).toBeHidden();
	await expect(
		crumb(page, `${seed.organization.name} / ${iterations.name}`),
	).toBeVisible();
	expect(documents).toEqual([]);
});

// Kills a version 0 project sent anywhere but its root in the console, a pick
// that loads a new document for it, and an Enter that does not open the
// search's first match.
test("a version 0 project opens in the console", async ({ page }) => {
	await arrive(page);
	const documents: string[] = [];
	page.on("request", (request) => {
		if (request.resourceType() === "document") {
			documents.push(request.url());
		}
	});
	await crumb(page).click();

	await search(page).fill(version_zero.name);
	await search(page).press("Enter");

	await expect(page).toHaveURL(consolePath(version_zero.slug));
	await expect(
		crumb(page, `${seed.organization.name} / ${version_zero.name}`),
	).toBeVisible();
	await expect(
		page.getByRole("heading", { level: 1, name: "6 runs" }),
	).toBeVisible();
	expect(documents).toEqual([]);
});

// Kills the reader's projects asked for on every load, and a hover that starts
// neither them nor the switcher's code.
test("a cold load asks for the reader's projects only once the crumb is near", async ({
	page,
}) => {
	const asked: string[] = [];
	page.on("request", (request) => {
		if (request.url().startsWith(BOOTSTRAP)) {
			asked.push(request.url());
		}
	});
	await arrive(page);
	expect(asked).toEqual([]);

	const bootstrap = page.waitForRequest((request) =>
		request.url().startsWith(BOOTSTRAP),
	);
	const code = page.waitForRequest(
		(request) => request.resourceType() === "script",
	);
	await crumb(page).hover();
	await Promise.all([bootstrap, code]);
	await crumb(page).click();
	await expect(option(page, hashbrown.name)).toBeVisible();
});

// Kills a crumb a keyboard reader reaches without starting the reader's projects.
test("tabbing to the crumb asks for the reader's projects", async ({
	page,
}) => {
	await arrive(page);
	const bootstrap = page.waitForRequest((request) =>
		request.url().startsWith(BOOTSTRAP),
	);

	for (let tab = 0; tab < 4; tab++) {
		await page.keyboard.press("Tab");
		if (
			await page.evaluate(() =>
				document.activeElement?.classList.contains("crumbbtn"),
			)
		) {
			break;
		}
	}

	await expect(crumb(page)).toBeFocused();
	await bootstrap;
});

// Kills a switcher that waits on the API for projects the reader has seen.
test("a returning reader's switcher opens from the cache", async ({ page }) => {
	await arrive(page);
	await crumb(page).click();
	await expect(option(page, hashbrown.name)).toBeVisible();
	// The cache saves a second after it changes.
	await settle(page, 1_500);
	await page.reload();
	await settle(page);

	await page.route(`${seed.api_url}/**`, () => {});
	await crumb(page).click();

	await expect(option(page, hashbrown.name)).toBeVisible();
	await page.unrouteAll({ behavior: "ignoreErrors" });
});
