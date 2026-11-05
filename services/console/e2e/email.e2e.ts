import type { APIRequestContext, Page } from "@playwright/test";
import type { JsonAuthUser, JsonUser } from "../src/types/bencher";
import {
	USER_KEY,
	expect,
	seed,
	settle,
	signedIn,
	signedOut,
	test,
} from "./fixtures";
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
		const email = page.getByRole("textbox", { name: "Email" });
		await expect(email).toHaveValue(reader.user.email);
		await email.fill(newEmail);
		await page.getByRole("button", { name: "Save changes" }).click();
		await expect(
			page.getByRole("status").filter({ hasText: "Confirm link" }),
		).toHaveText("Confirm link sent");
		await expect(page).toHaveURL(settings(reader.user.slug));
		await expect(email).toHaveValue(reader.user.email);
		expect((await storedUser(page)).token).toBe(reader.token);

		const link = await mailedLink(newEmail, "Confirm Bencher Email Change");
		expect(link.pathname).toBe("/auth/email");
		await page.goto(link.href);
		await page.getByRole("button", { name: "Change email" }).click();
		await expect(page).toHaveURL(settings(reader.user.slug));
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
			newEmail,
		);
		const stored = await storedUser(page);
		expect(stored.user.email).toBe(newEmail);
		expect(stored.token).not.toBe(reader.token);
	});
});

test.describe("a confirm link", () => {
	test.use({ storageState: signedOut });

	// Kills a confirm sent on load, seconds later, or on a scanner's Enter, signed out or holding the link user's session, which another link can plant.
	test("opened signed out or as the same user changes nothing before the press", async ({
		page,
		newContext,
		request,
	}) => {
		const reader = await signUp(request);
		const newEmail = `new-${reader.user.email}`;
		const link = await askForEmail(request, reader, newEmail);
		const context = await newContext({ storageState: signedIn(reader) });
		const same = await context.newPage();
		const puts = [sentPuts(page), sentPuts(same)];

		for (const opened of [page, same]) {
			await opened.goto(link.href);
			await expect(
				opened.getByRole("button", { name: "Change email" }),
			).toBeVisible();
		}
		for (const opened of [page, same]) {
			await opened.keyboard.press("Enter");
		}
		await page.waitForTimeout(5_000);
		expect(puts).toEqual([[], []]);
		for (const opened of [page, same]) {
			await expect(opened.getByRole("status")).toBeEmpty();
		}
		expect(await apiEmail(request, reader)).toBe(reader.user.email);
		expect((await storedUser(page)).token).toBeUndefined();
		expect((await storedUser(same)).token).toBe(reader.token);
	});

	// Kills a button that does nothing, and a press that confirms but does not sign the reader in.
	test("opened signed out signs the reader in with the new email after the press", async ({
		page,
		request,
	}) => {
		const reader = await signUp(request);
		const newEmail = `new-${reader.user.email}`;
		const link = await askForEmail(request, reader, newEmail);

		await page.goto(link.href);
		await page.getByRole("button", { name: "Change email" }).click();
		await expect(page).toHaveURL(settings(reader.user.slug));
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
			newEmail,
		);
		expect((await storedUser(page)).user.email).toBe(newEmail);
		expect(await apiEmail(request, reader)).toBe(newEmail);
	});

	// Kills a confirm page that replaces another user's stored session with the confirmed user's.
	test("opened while another user is signed in keeps that user signed in after the press", async ({
		newContext,
		request,
	}) => {
		const reader = await signUp(request);
		const newEmail = `new-${reader.user.email}`;
		const link = await askForEmail(request, reader, newEmail);
		const context = await newContext({ storageState: signedIn(seed.member) });
		const page = await context.newPage();

		await page.goto(link.href);
		const button = page.getByRole("button", { name: "Change email" });
		await expect(button).toBeVisible();
		await expect(page.getByRole("status")).toBeEmpty();
		await button.click();
		await expect(
			page
				.getByRole("status")
				.getByRole("heading", { name: "Your email has changed" }),
		).toBeVisible();
		await expect(page).toHaveURL(/\/auth\/email\?/);
		expect((await storedUser(page)).token).toBe(seed.member.token);
		expect(await apiEmail(request, reader)).toBe(newEmail);
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
		await page.getByRole("button", { name: "Change email" }).click();
		await expect(
			page
				.getByRole("status")
				.getByRole("heading", { name: "This link has expired" }),
		).toBeVisible();
	});

	// Kills a page with no invalid state, one that skips the user or token check, and one that offers the press first.
	test("with a malformed user or token says the link is not valid", async ({
		page,
	}) => {
		const puts = sentPuts(page);
		for (const params of [
			{ user: "nope", token: seed.member.token },
			{ user: seed.member.user.uuid, token: "nope" },
		]) {
			await page.goto(`/auth/email?${new URLSearchParams(params)}`);
			await expect(
				page.getByRole("heading", { name: "This link is not valid" }),
			).toBeVisible();
			await expect(
				page.getByRole("button", { name: "Change email" }),
			).toHaveCount(0);
			await expect(page.getByRole("status")).toBeEmpty();
		}
		await settle(page);
		expect(puts).toEqual([]);
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

/** Every PUT the page sends from now on, through any redirect. */
const sentPuts = (page: Page) => {
	const puts: string[] = [];
	page.on("request", (sent) => {
		if (sent.method() === "PUT") {
			puts.push(sent.url());
		}
	});
	return puts;
};

/** The reader's email as the API holds it, read as the server admin. */
const apiEmail = async (request: APIRequestContext, reader: JsonAuthUser) => {
	const response = await request.get(
		`${seed.api_url}/v0/users/${reader.user.uuid}`,
		{ headers: { Authorization: `Bearer ${seed.admin.token}` } },
	);
	expect(response.status()).toBe(200);
	return ((await response.json()) as JsonUser).email;
};

const storedUser = (page: Page): Promise<JsonAuthUser> =>
	page.evaluate(
		(key) => JSON.parse(localStorage.getItem(key) ?? "{}"),
		USER_KEY,
	);
