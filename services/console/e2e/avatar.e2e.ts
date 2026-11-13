import type { Page } from "@playwright/test";
import { createMember } from "./api";
import {
	ORGANIZATION_KEY,
	READER_KEY,
	SESSION_COOKIE,
	SHELL_KEY,
	VERSION_KEY,
	expect,
	consolePath,
	persistedQueries,
	seed,
	signedIn,
	tabRow,
	test,
} from "./fixtures";
import { inSession } from "./session";

const { hashbrown, iterations } = seed.projects;
const RUNS = consolePath(hashbrown.slug, "runs");

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

// Kills a Sign out that leaves the session alive or its cookie in the
// browser, keeps the reader, their organization, their names, their landing,
// or their cache in this browser, or sends the next reader to this one's page.
test("Sign out ends the session and forgets all this browser remembers for the reader, then asks for a sign in", async ({
	newContext,
}) => {
	// A reader of the test's own: signing out ends the session for every page that shares it.
	const reader = await createMember(seed.organization, seed.member);
	const context = await newContext({ storageState: signedIn(reader) });
	const page = await context.newPage();
	await page.goto(RUNS);
	await page.evaluate(
		(key) => localStorage.setItem(key, JSON.stringify({ slug: "x" })),
		ORGANIZATION_KEY,
	);
	await expect
		.poll(() => persistedQueries(page, reader.user.uuid))
		.toBeGreaterThan(0);
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
			[READER_KEY, ORGANIZATION_KEY, SHELL_KEY, VERSION_KEY],
		),
	).toEqual([]);
	await expect.poll(() => persistedQueries(page, reader.user.uuid)).toBe(0);
	expect(
		(await context.cookies()).filter(({ name }) => name === SESSION_COOKIE),
	).toEqual([]);
	expect((await inSession(reader, "GET", "/v0/auth/session")).status).toBe(401);
});

test.describe("on a phone", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a phone with no way to switch projects, and a pick from the sheet
	// that loads a new document rather than moving in the app.
	test("Switch project opens the switcher as a sheet, and a pick moves in the app", async ({
		page,
	}) => {
		const search = () =>
			sheet(page).getByRole("combobox", { name: "Search projects" });
		await page.goto(RUNS);
		await openMenu(page);
		await page.getByRole("menuitem", { name: /^Switch project/ }).click();
		await expect(search()).toBeFocused();
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

		await expect(page).toHaveURL(consolePath(iterations.slug));
		await expect(sheet(page)).toBeHidden();
		expect(documents).toEqual([]);
	});
});
