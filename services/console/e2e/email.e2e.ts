import type { APIRequestContext, Page } from "@playwright/test";
import type { JsonUser } from "../src/types/bencher";
import {
	READER_KEY,
	type Reader,
	expect,
	hintOf,
	seed,
	settle,
	signedIn,
	signedOut,
	test,
} from "./fixtures";
import { mailedLink } from "./mail";
import { inSession, signUp } from "./session";

const settings = (slug: string) => `/console/users/${slug}/settings`;
// Shaped as a confirm link's token, and signed by no key the API holds.
const UNSIGNED = `eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ4In0.${"A".repeat(43)}`;

/** Resolves once `page` asks for the door, which a confirmed link lands on. */
const opensDoor = (page: Page) =>
	page.waitForRequest(
		(sent) =>
			sent.isNavigationRequest() &&
			new URL(sent.url()).pathname === "/auth/login",
	);

test.describe("a new email", () => {
	// Kills a profile that shows a new email as changed before it is confirmed,
	// a confirm link that does not change it or leaves its token in the
	// address, and a console that shows the old email from its cache after.
	test("waits for its confirm link, which changes it and lands on the door", async ({
		newContext,
	}) => {
		const reader = await signUp();
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

		const link = await mailedLink(newEmail, "Confirm Bencher Email Change");
		expect(link.pathname).toBe("/auth/email");
		await page.goto(link.href);
		// The link's token leaves the address before anything can read it there.
		await expect(page).toHaveURL("/auth/email");
		const door = opensDoor(page);
		await page.getByRole("button", { name: "Change email" }).click();
		await door;
		await page.waitForURL(/\/console\//);
		expect(await hint(page)).toBe(hintOf(reader));
		await page.goto(settings(reader.user.slug));
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
			newEmail,
		);
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
		const reader = await signUp();
		const newEmail = `new-${reader.user.email}`;
		const link = await askForEmail(reader, newEmail);
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
		expect(await hint(page)).toBeNull();
		expect(await hint(same)).toBe(hintOf(reader));
	});

	// Kills a button that does nothing, and a press that confirms but lands anywhere but the door.
	test("opened signed out confirms after the press and lands on the door", async ({
		page,
		request,
	}) => {
		const reader = await signUp();
		const newEmail = `new-${reader.user.email}`;
		const link = await askForEmail(reader, newEmail);

		await page.goto(link.href);
		await page.getByRole("button", { name: "Change email" }).click();
		await expect(page).toHaveURL("/auth/login");
		await expect(
			page.getByRole("heading", { level: 1, name: "Sign in to Bencher" }),
		).toBeVisible();
		expect(await hint(page)).toBeNull();
		expect(await apiEmail(request, reader)).toBe(newEmail);
	});

	// Kills a confirm page that signs the browser in as the confirmed user over another user.
	test("opened while another user is signed in keeps that user signed in after the press", async ({
		newContext,
		request,
	}) => {
		const reader = await signUp();
		const newEmail = `new-${reader.user.email}`;
		const link = await askForEmail(reader, newEmail);
		const context = await newContext({ storageState: signedIn(seed.member) });
		const page = await context.newPage();

		await page.goto(link.href);
		const button = page.getByRole("button", { name: "Change email" });
		await expect(button).toBeVisible();
		await expect(page.getByRole("status")).toBeEmpty();
		const door = opensDoor(page);
		await button.click();
		await door;
		await page.waitForURL(/\/console\//);
		expect(await hint(page)).toBe(hintOf(seed.member));
		expect(await apiEmail(request, reader)).toBe(newEmail);
	});

	// Kills a confirm page that does not show a 401 answer as an expired link.
	test("with a token the API refuses says the link has expired", async ({
		page,
	}) => {
		const params = new URLSearchParams({
			user: seed.member.user.uuid,
			token: UNSIGNED,
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
			{ user: "nope", token: UNSIGNED },
			{ user: seed.member.user.uuid, token: "nope" },
		]) {
			await page.goto(`/auth/email?${new URLSearchParams(params)}`);
			await expect(
				page.getByRole("heading", { name: "This link is not valid" }),
			).toBeVisible();
			await expect(
				page.getByRole("button", { name: "Change email" }),
			).toHaveCount(0);
			await expect(page.getByRole("status")).toHaveCount(0);
		}
		await settle(page);
		expect(puts).toEqual([]);
	});
});

/** Ask for `newEmail` in the reader's session, as only a session may, and return the confirm link it sends. */
const askForEmail = async (reader: Reader, newEmail: string) => {
	const asked = await inSession(
		reader,
		"PATCH",
		`/v0/users/${reader.user.uuid}`,
		{
			email: newEmail,
		},
	);
	expect(asked.status).toBe(200);
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
const apiEmail = async (request: APIRequestContext, reader: Reader) => {
	const response = await request.get(
		`${seed.api_url}/v0/users/${reader.user.uuid}`,
		{ headers: { Authorization: `Bearer ${seed.admin.key}` } },
	);
	expect(response.status()).toBe(200);
	return ((await response.json()) as JsonUser).email;
};

const hint = (page: Page) =>
	page.evaluate((key) => localStorage.getItem(key), READER_KEY);
