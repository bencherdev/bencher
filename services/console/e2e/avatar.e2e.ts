import type { Page } from "@playwright/test";
import { axeInBothThemes } from "./axe";
import {
	ORGANIZATION_KEY,
	SHELL_KEY,
	USER_KEY,
	VERSION_KEY,
	expect,
	nextPath,
	persistedQueries,
	seed,
	signedIn,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown, iterations } = seed.projects;
const RUNS = nextPath(hashbrown.slug, "runs");

test.use({ storageState: signedIn(seed.member) });

const avatar = (page: Page) =>
	page.getByRole("banner").getByRole("button", { name: "Account" });
const menu = (page: Page) => page.getByRole("menu", { name: "Account" });
const sheet = (page: Page) =>
	page.getByRole("dialog", { name: "Switch project" });

/** Runs of Hashbrown, drawn, with the avatar's menu open. */
const openMenu = async (page: Page) => {
	await expect(
		tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
	).toBeVisible();
	await avatar(page).click();
	await expect(menu(page)).toBeVisible();
};

// Kills a menu that fails axe in either theme, on a desktop or a phone.
test("the avatar's menu passes axe in both themes", async ({ page }) => {
	await axeInBothThemes(
		page,
		RUNS,
		() => expect(menu(page)).toBeVisible(),
		() => openMenu(page),
	);
});

// Kills a Sign out that keeps the reader, their organization, their names,
// their landing, or their cache in this browser, or sends the next reader to
// this one's page.
test("Sign out forgets the reader and all this browser remembers for them, then asks for a sign in", async ({
	page,
}) => {
	await page.goto(RUNS);
	await page.evaluate(
		(key) => localStorage.setItem(key, JSON.stringify({ slug: "x" })),
		ORGANIZATION_KEY,
	);
	await expect.poll(() => persistedQueries(page)).toBeGreaterThan(0);
	await expect
		.poll(() =>
			page.evaluate(
				(keys) => keys.filter((key) => localStorage.getItem(key) === null),
				[SHELL_KEY, VERSION_KEY],
			),
		)
		.toEqual([]);
	await openMenu(page);

	await page.getByRole("menuitem", { name: "Sign out" }).click();

	await expect(page).toHaveURL(
		(url) => url.pathname === "/auth/login" && !url.searchParams.has("back"),
	);
	expect(
		await page.evaluate(
			(keys) => keys.filter((key) => localStorage.getItem(key) !== null),
			[USER_KEY, ORGANIZATION_KEY, SHELL_KEY, VERSION_KEY],
		),
	).toEqual([]);
	await expect.poll(() => persistedQueries(page)).toBe(0);
});

test.describe("on a phone", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a phone with no way to switch projects, a sheet that fails axe, and
	// a pick from the sheet that loads a new document rather than moving in the app.
	test("Switch project opens the switcher as a sheet, and a pick moves in the app", async ({
		page,
	}) => {
		const search = () =>
			sheet(page).getByRole("combobox", { name: "Search projects" });
		await axeInBothThemes(
			page,
			RUNS,
			() => expect(search()).toBeFocused(),
			async () => {
				await openMenu(page);
				await page.getByRole("menuitem", { name: /^Switch project/ }).click();
			},
		);
		const documents: string[] = [];
		page.on("request", (request) => {
			if (request.resourceType() === "document") {
				documents.push(request.url());
			}
		});

		await search().fill(iterations.name);
		await sheet(page)
			.getByRole("option", { name: new RegExp(`^${iterations.name}`) })
			.click();

		await expect(page).toHaveURL(nextPath(iterations.slug));
		await expect(sheet(page)).toBeHidden();
		expect(documents).toEqual([]);
	});
});
