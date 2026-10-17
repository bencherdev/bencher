import type {
	BrowserContext,
	BrowserContextOptions,
	Page,
} from "@playwright/test";
import type { JsonAuthUser } from "../src/types/bencher";
import { createReader, readUser, unique } from "./api";
import { axeInBothThemes } from "./axe";
import {
	VERSION_KEY,
	expect,
	nextPath,
	seed,
	signedIn,
	test,
} from "./fixtures";

const { hashbrown, version_zero } = seed.projects;

const profilePath = (user: string) => `/next/console/users/${user}/settings`;
const classicProfile = (user: string) => `/console/users/${user}/settings`;
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
		const avatar = page.getByRole("link", { name: "Account" });

		await textbox(page, "Slug").fill(slug);
		await save(page).click();

		await expect(page).toHaveURL(profilePath(slug));
		await expect(avatar).toHaveAttribute("href", profilePath(slug));
		await page.reload();
		await expect(textbox(page, "Slug")).toHaveValue(slug);
		await expect(avatar).toHaveAttribute("href", profilePath(slug));
	});

	// Kills an email change sent before the reader confirms the revocation, a
	// count of legacy tokens read from anything but the tokens list, and a
	// session kept once the email it was signed in with is gone.
	test("an email change with legacy tokens confirms before it is sent, then signs out", async ({
		newContext,
		request,
	}) => {
		const reader = await createReader(request);
		const page = await readerPage(newContext, reader);
		const { slug, uuid } = reader.user;
		await twoLegacyTokens(page, slug);
		// A user key cannot change an email, so the API keeps it and answers as if it had.
		const sent: unknown[] = [];
		await page.route(`${seed.api_url}/v0/users/${uuid}`, async (route) => {
			if (route.request().method() !== "PATCH") {
				await route.fallback();
				return;
			}
			sent.push(route.request().postDataJSON());
			const response = await route.fetch({ postData: "{}" });
			await route.fulfill({
				response,
				json: { ...reader.user, email: "courage@bagge.example" },
			});
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
		await expect(page).toHaveURL(/\/auth\/login\?/);
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

	// Kills an avatar that leaves the console's app for Profile, reloading
	// the page.
	test("the avatar opens Profile without a reload", async ({ page }) => {
		await page.goto(nextPath(hashbrown.slug, "settings"));
		await expect(textbox(page, "Name")).toHaveValue(hashbrown.name);
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});

		await page.getByRole("link", { name: "Account" }).click();

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

	// Kills the classic shortcut to Profile lost under /next/.
	test("opens Profile", async ({ page }) => {
		await page.goto(`/next/console/users/${seed.member.user.slug}`);

		await expect(page).toHaveURL(profilePath(seed.member.user.slug));
		await expect(heading(page)).toBeVisible();
	});
});

const remembered = (versions: Record<string, number>) =>
	signedIn(seed.member, { [VERSION_KEY]: JSON.stringify(versions) });

test.describe("the classic Profile, for a reader with a version 1 project", () => {
	test.use({ storageState: remembered({ [hashbrown.slug]: 1 }) });

	// Kills a classic Profile that keeps a reader the new one serves.
	test("sends the reader to Profile under /next/", async ({ page }) => {
		await page.goto(classicProfile(seed.member.user.slug));

		await expect(page).toHaveURL(profilePath(seed.member.user.slug));
		await expect(heading(page)).toBeVisible();
	});
});

test.describe("the classic Profile, for a reader of version 0 projects only", () => {
	test.use({ storageState: remembered({ [version_zero.slug]: 0 }) });

	// Kills a classic Profile that sends away a reader the new one does not serve.
	test("keeps the reader", async ({ page }) => {
		await page.goto(classicProfile(seed.member.user.slug));
		await page.waitForLoadState("networkidle");

		await expect(page).toHaveURL(classicProfile(seed.member.user.slug));
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

	test("Profile's email confirmation passes axe in both themes", async ({
		page,
	}) => {
		const { slug } = seed.member.user;
		await twoLegacyTokens(page, slug);
		await axeInBothThemes(
			page,
			profilePath(slug),
			async () => {},
			async () => {
				await expect(
					page.getByRole("link", { name: "2 legacy tokens" }),
				).toBeVisible();
				await textbox(page, "Email").fill("muriel@bagge.example");
				await save(page).click();
				await expect(page.getByRole("alertdialog")).toBeVisible();
			},
		);
	});
});
