import type { APIRequestContext, Page } from "@playwright/test";
import type { JsonAuthUser } from "../src/types/bencher";
import { USER_KEY, expect, seed, signedIn, signedOut, test } from "./fixtures";
import { mailedLink, signUp } from "./mail";

const settings = (slug: string) => `/console/users/${slug}/settings`;

test.describe("a new email", () => {
	// Kills a profile that shows a new email as changed before it is confirmed,
	// and a confirm link that does not change it or refresh the stored user.
	test("waits for its confirm link, which changes it and signs the reader in again", async ({
		newContext,
		request,
	}) => {
		const reader = await signUp(request);
		const context = await newContext({ storageState: signedIn(reader) });
		const page = await context.newPage();
		const newEmail = `new-${reader.user.email}`;

		await page.goto(settings(reader.user.slug));
		const card = page.locator("#Email");
		await card.getByRole("button", { name: "Update" }).click();
		await card.getByRole("textbox").fill(newEmail);
		await card.getByRole("button", { name: "Save" }).click();
		await expect(
			page.getByText(`We've sent a confirm link to ${newEmail}.`),
		).toBeVisible();
		await expect(page).toHaveURL(new RegExp(settings(reader.user.slug)));
		await expect(card.getByRole("textbox")).toHaveValue(reader.user.email);

		const link = await mailedLink(newEmail, "Confirm Bencher Email Change");
		expect(link.pathname).toBe("/auth/email");
		await page.goto(link.href);
		await expect(
			page.getByText(`Your email is now ${newEmail}.`),
		).toBeVisible();
		await expect(page).toHaveURL(new RegExp(settings(reader.user.slug)));
		await expect(page.locator("#Email").getByRole("textbox")).toHaveValue(
			newEmail,
		);
		const stored = await storedUser(page);
		expect(stored.user.email).toBe(newEmail);
		expect(stored.token).not.toBe(reader.token);
	});
});

test.describe("a confirm link", () => {
	test.use({ storageState: signedOut });

	// Kills a confirm page that treats a reader with no stored session as another user.
	test("opened signed out signs the reader in with the new email", async ({
		page,
		request,
	}) => {
		const reader = await signUp(request);
		const newEmail = `new-${reader.user.email}`;
		const link = await askForEmail(request, reader, newEmail);

		await page.goto(link.href);
		await expect(
			page.getByText(`Your email is now ${newEmail}.`),
		).toBeVisible();
		await expect(page).toHaveURL(new RegExp(settings(reader.user.slug)));
		expect((await storedUser(page)).user.email).toBe(newEmail);
	});

	// Kills a confirm page that replaces another user's stored session with the confirmed user's.
	test("opened while another user is signed in keeps that user signed in", async ({
		newContext,
		request,
	}) => {
		const reader = await signUp(request);
		const newEmail = `new-${reader.user.email}`;
		const link = await askForEmail(request, reader, newEmail);
		const context = await newContext({ storageState: signedIn(seed.member) });
		const page = await context.newPage();

		await page.goto(link.href);
		await expect(
			page.getByRole("heading", { name: "Your email has changed" }),
		).toBeVisible();
		await expect(page).toHaveURL(/\/auth\/email\?/);
		expect((await storedUser(page)).token).toBe(seed.member.token);
	});

	// Kills a confirm page that does not show a 401 answer as an expired link.
	test("with a token the API refuses says the link has expired", async ({
		page,
	}) => {
		// The API cannot decode a session token as a confirm link's, so it answers 401.
		const params = new URLSearchParams({
			user: seed.member.user.uuid,
			token: seed.member.token,
		});
		await page.goto(`/auth/email?${params}`);
		await expect(
			page.getByRole("heading", { name: "This link has expired" }),
		).toBeVisible();
	});
});

/** Ask for `newEmail` as the reader and return the confirm link it sends. */
const askForEmail = async (
	request: APIRequestContext,
	reader: JsonAuthUser,
	newEmail: string,
) => {
	const response = await request.patch(
		`${seed.api_url}/v0/users/${reader.user.uuid}`,
		{
			headers: { Authorization: `Bearer ${reader.token}` },
			data: { email: newEmail },
		},
	);
	expect(response.status()).toBe(200);
	return mailedLink(newEmail, "Confirm Bencher Email Change");
};

const storedUser = (page: Page): Promise<JsonAuthUser> =>
	page.evaluate(
		(key) => JSON.parse(localStorage.getItem(key) ?? "{}"),
		USER_KEY,
	);
