import type {
	BrowserContext,
	BrowserContextOptions,
	Page,
} from "@playwright/test";
import type { JsonAuthUser } from "../src/types/bencher";
import { createReader, readUser, unique } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, consolePath, seed, signedIn, test } from "./fixtures";

const { hashbrown } = seed.projects;

const profilePath = (user: string) => `/console/users/${user}/settings`;
const heading = (page: Page) =>
	page.getByRole("heading", { level: 1, name: "Profile" });
const textbox = (page: Page, name: string) =>
	page.getByRole("textbox", { name });
const save = (page: Page) => page.getByRole("button", { name: "Save changes" });

/** The API says `user` has two legacy tokens, since no API makes one any more. */
const twoLegacyTokens = (page: Page, user: string) =>
	page.route(`${seed.api_url}/v0/users/${user}/tokens?*`, async (route) => {
		const response = await route.fetch();
		await route.fulfill({
			response,
			headers: { ...response.headers(), "x-total-count": "2" },
		});
	});

/** A page of its own for a new reader, whom each test may change. */
const readerPage = async (
	newContext: (options?: BrowserContextOptions) => Promise<BrowserContext>,
	reader: JsonAuthUser,
) => (await newContext({ storageState: signedIn(reader) })).newPage();

test.describe("Profile", () => {
	// Kills a save that never reaches the API, and one sent for another user.
	test("a reader's new name reaches the API and survives a reload", async ({
		newContext,
		request,
	}) => {
		const reader = await createReader(request);
		const page = await readerPage(newContext, reader);
		await page.goto(profilePath(reader.user.slug));
		const renamed = unique("Courage");

		await textbox(page, "Name").fill(renamed);
		await save(page).click();

		await expect(page.getByRole("main").getByRole("status")).toHaveText(
			"Saved",
		);
		expect((await readUser(request, reader.user.slug, reader.token)).name).toBe(
			renamed,
		);
		await page.reload();
		await expect(textbox(page, "Name")).toHaveValue(renamed);
	});

	// Kills a new slug the console forgets, in this session or after a reload,
	// which leaves the avatar on a Profile that no longer answers.
	test("a new slug moves Profile and the avatar, and survives a reload", async ({
		newContext,
		request,
	}) => {
		const reader = await createReader(request);
		const page = await readerPage(newContext, reader);
		await page.goto(profilePath(reader.user.slug));
		const slug = `${reader.user.slug}-x`;
		/** Where the avatar's menu leads for Profile, closing it again. */
		const profileEntry = async () => {
			await page.getByRole("button", { name: "Account" }).click();
			const href = await page
				.getByRole("menuitem", { name: "Profile" })
				.getAttribute("href");
			await page.keyboard.press("Escape");
			return href;
		};

		await textbox(page, "Slug").fill(slug);
		await save(page).click();

		await expect(page).toHaveURL(profilePath(slug));
		expect(await profileEntry()).toBe(profilePath(slug));
		await page.reload();
		await expect(textbox(page, "Slug")).toHaveValue(slug);
		expect(await profileEntry()).toBe(profilePath(slug));
	});

	// Kills an email change sent before the reader confirms the revocation, a
	// count of legacy tokens read from anything but the tokens list, and a
	// reader signed out while the change waits for its confirm link.
	test("an email change with legacy tokens confirms before it is sent, then waits for its link", async ({
		newContext,
		request,
	}) => {
		const reader = await createReader(request);
		const page = await readerPage(newContext, reader);
		const { slug, uuid } = reader.user;
		await twoLegacyTokens(page, slug);
		// A user key cannot change an email; the API holds a session's change for
		// its confirm link and answers with the current email, as here.
		const sent: unknown[] = [];
		await page.route(`${seed.api_url}/v0/users/${uuid}`, async (route) => {
			if (route.request().method() !== "PATCH") {
				await route.fallback();
				return;
			}
			sent.push(route.request().postDataJSON());
			const response = await route.fetch({ postData: "{}" });
			await route.fulfill({ response, json: reader.user });
		});
		await page.goto(profilePath(slug));

		await expect(
			page.getByRole("link", { name: "2 legacy tokens" }),
		).toBeVisible();
		await textbox(page, "Email").fill("courage@bagge.example");
		await save(page).click();
		const dialog = page.getByRole("alertdialog");
		await expect(dialog).toContainText("Your 2 legacy tokens will be revoked.");
		expect(sent).toEqual([]);

		await dialog.getByRole("button", { name: "Change email" }).click();
		await expect(
			page.getByRole("status").filter({ hasText: "Confirm link" }),
		).toHaveText("Confirm link sent to courage@bagge.example");
		await expect(page).toHaveURL(profilePath(slug));
		expect(sent).toEqual([{ email: "courage@bagge.example" }]);
	});

	// Kills a link to legacy tokens drawn when there are none.
	test("a reader without legacy tokens sees no link to them", async ({
		newContext,
		request,
	}) => {
		const reader = await createReader(request);
		const page = await readerPage(newContext, reader);
		await page.goto(profilePath(reader.user.slug));

		await expect(textbox(page, "Email")).toHaveValue(reader.user.email);
		await page.waitForLoadState("networkidle");
		await expect(page.getByRole("link", { name: /legacy token/ })).toHaveCount(
			0,
		);
	});
});

test.describe("from a project page", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills an avatar menu that leaves the console's app for Profile, reloading
	// the page.
	test("the avatar's menu opens Profile from a project without a reload", async ({
		page,
	}) => {
		await page.goto(consolePath(hashbrown.slug, "settings"));
		await expect(textbox(page, "Name")).toHaveValue(hashbrown.name);
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});

		await page.getByRole("button", { name: "Account" }).click();
		await page.getByRole("menuitem", { name: "Profile" }).click();

		await expect(page).toHaveURL(profilePath(seed.member.user.slug));
		await expect(textbox(page, "Name")).toHaveValue(seed.member.user.name);
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});
});

test.describe("the account's root", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills the classic account root left with no page.
	test("opens Profile", async ({ page }) => {
		await page.goto(`/console/users/${seed.member.user.slug}`);

		await expect(page).toHaveURL(profilePath(seed.member.user.slug));
		await expect(heading(page)).toBeVisible();
	});
});

test.describe("accessibility", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills contrast, names, and roles that fail axe in either theme.
	test("Profile passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(page, profilePath(seed.member.user.slug), () =>
			expect(textbox(page, "Name")).toHaveValue(seed.member.user.name),
		);
	});
});
