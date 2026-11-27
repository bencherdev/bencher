import type {
	APIRequestContext,
	BrowserContext,
	BrowserContextOptions,
	Page,
} from "@playwright/test";
import type { JsonUser } from "../src/types/bencher";
import { axeInBothThemes } from "./axe";
import { typeCode } from "./door";
import {
	READER_KEY,
	type Reader,
	expect,
	hintOf,
	persistedQueries,
	provedAgo,
	seed,
	signedIn,
	signedOut,
	test,
} from "./fixtures";
import { mailMark, mailed, mailedLink, mailedSignIn } from "./mail";
import { inSession, signInByEmail, signUp } from "./session";

const HOUR = 60 * 60;
const CONFIRM = "Confirm Bencher Email Change";
const ASKED = "Bencher Email Change Asked For";
const PROFILE = "/console/users/settings";
const ELSEWHERE =
	"Open this link in the browser where you asked for the change.";
const settings = (slug: string) => `/console/users/${slug}/settings`;

test.describe("a new email", () => {
	// Kills an email change that skips the fresh sign-in, applies before its
	// link, tells the old address nothing or gives it a link, carries its secret
	// anywhere but the fragment, leaves it in the address or puts it in a
	// request's URL, or keeps any session of the reader's but the one that
	// asked.
	test("asks to sign in again, tells the old address, and its link confirms in the browser that asked, keeping only its session", async ({
		newContext,
		request,
	}) => {
		const reader = await signUp();
		const other = await signInByEmail(reader.user.email);
		provedAgo(reader.session, HOUR);
		const asking = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		const newEmail = `new-${reader.user.email}`;

		await asking.goto(settings(reader.user.slug));
		const email = asking.getByRole("textbox", { name: "Email" });
		await expect(email).toHaveValue(reader.user.email);
		await email.fill(newEmail);
		const mark = mailMark();
		await asking.getByRole("button", { name: "Save changes" }).click();
		const again = asking.getByRole("dialog", { name: "Sign in again" });
		await again.getByRole("button", { name: "Email a code" }).click();
		const { code } = await mailedSignIn(reader.user.email, mark);
		await typeCode(asking, code);
		await expect(
			asking.getByRole("status").filter({ hasText: "Confirm link" }),
		).toHaveText("Confirm link sent");
		await expect(email).toHaveValue(reader.user.email);
		expect(await apiEmail(request, reader)).toBe(reader.user.email);

		const notice = await mailed(reader.user.email, ASKED);
		expect(notice).toContain(newEmail);
		expect(notice.match(/https?:\/\/[^\s)]+/g)).toEqual([
			`${seed.console_url}/help`,
		]);
		const link = await mailedLink(newEmail, CONFIRM);
		expect(link.pathname).toBe("/auth/email");
		expect(link.search).toBe("");
		const secret = new URLSearchParams(link.hash.slice(1)).get("secret") ?? "";
		expect(secret).toMatch(/^[\w-]{43}$/);

		const urls: string[] = [];
		asking.on("request", (sent) => urls.push(sent.url()));
		await asking.goto(link.href);
		await expect(asking).toHaveURL("/auth/email");
		await expect(asking.locator(".kv")).toHaveText([
			`from${reader.user.email}`,
			`to${newEmail}`,
		]);
		await asking.getByRole("button", { name: "Change email" }).click();
		await asking.waitForURL(settings(reader.user.slug));
		expect(urls.filter((url) => url.includes(secret))).toEqual([]);

		expect(await apiEmail(request, reader)).toBe(newEmail);
		const kept = await inSession<{ user: JsonUser }>(
			reader,
			"GET",
			"/v0/auth/session",
		);
		expect(kept.status).toBe(200);
		expect(kept.data.user.email).toBe(newEmail);
		expect((await inSession(other, "GET", "/v0/auth/session")).status).toBe(
			401,
		);
	});
});

test.describe("a confirm link", () => {
	test.use({ storageState: signedOut });

	// Kills a confirm sent on load, seconds later, or on a scanner's Enter,
	// signed out or in the asking reader's own browser.
	test("changes nothing before the press, signed out or signed in", async ({
		page,
		newContext,
		request,
	}) => {
		const reader = await signUp();
		const link = await askForEmail(reader, `new-${reader.user.email}`);
		const same = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		const confirms = [sentConfirms(page), sentConfirms(same)];

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
		expect(confirms).toEqual([[], []]);
		expect(await apiEmail(request, reader)).toBe(reader.user.email);
		expect(await hint(same)).toBe(hintOf(reader));
	});

	// Kills a confirm taken anywhere but the session that asked: signed out,
	// in another session of the reader's, or in another user's, which would
	// also sign that browser in as the reader; and a refusal that offers the
	// press again.
	test("opened anywhere but the browser that asked says where to open it and changes nothing", async ({
		newContext,
		request,
	}) => {
		const reader = await signUp();
		const other = await signInByEmail(reader.user.email);
		const link = await askForEmail(reader, `new-${reader.user.email}`);

		for (const [storageState, signedInAs] of [
			[signedOut, null],
			[signedIn({ ...reader, session: other.session }), hintOf(reader)],
			[signedIn(seed.member), hintOf(seed.member)],
		] as const) {
			const page = await (await newContext({ storageState })).newPage();
			await page.goto(link.href);
			await page.getByRole("button", { name: "Change email" }).click();
			await expect(page.getByRole("alert")).toHaveText(ELSEWHERE);
			await expect(
				page.getByRole("button", { name: "Change email" }),
			).toHaveCount(0);
			await expect(
				page.getByRole("link", { name: "Open Profile" }),
			).toHaveAttribute("href", PROFILE);
			await expect(page).toHaveURL("/auth/email");
			expect(await hint(page)).toBe(signedInAs);
		}
		expect(await apiEmail(request, reader)).toBe(reader.user.email);
		expect((await inSession(other, "GET", "/v0/auth/session")).status).toBe(
			200,
		);
	});

	// Kills a confirm that keeps the console's saved responses, so the reader's
	// own Profile still shows the email just replaced.
	test("confirmed in the reader's own browser, Profile shows the new email", async ({
		newContext,
		request,
	}) => {
		const reader = await signUp();
		const newEmail = `new-${reader.user.email}`;
		const page = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		await page.goto(settings(reader.user.slug));
		const email = page.getByRole("textbox", { name: "Email" });
		await expect(email).toHaveValue(reader.user.email);
		await expect
			.poll(() => persistedQueries(page, reader.user.uuid))
			.toBeGreaterThan(0);
		const link = await askForEmail(reader, newEmail);

		await page.goto(link.href);
		await page.getByRole("button", { name: "Change email" }).click();
		await page.waitForURL(settings(reader.user.slug));
		await expect(email).toHaveValue(newEmail);
		expect(await apiEmail(request, reader)).toBe(newEmail);
	});

	// Kills a second link opened in the tab of the first, which changes only
	// the address's fragment, left unread.
	test("a second link opened in the tab of the first shows its own change", async ({
		page,
	}) => {
		const first = await signUp();
		const firstEmail = `new-${first.user.email}`;
		const firstLink = await askForEmail(first, firstEmail);
		const second = await signUp();
		const secondEmail = `new-${second.user.email}`;
		const secondLink = await askForEmail(second, secondEmail);
		const terms = page.locator(".kv");

		await page.goto(firstLink.href);
		await expect(terms).toHaveText([
			`from${first.user.email}`,
			`to${firstEmail}`,
		]);
		await expect(page).toHaveURL("/auth/email");
		await page.goto(secondLink.href);
		await expect(terms).toHaveText([
			`from${second.user.email}`,
			`to${secondEmail}`,
		]);
	});

	// Kills each of board 05's states drawn wrong: an unknown or missing secret
	// not read as expired, a second use not read as used, a taken address not
	// named, a refused link that offers the press, and Open Profile that goes
	// anywhere but the reader's Profile.
	test("each state of board 05", async ({ page, newContext, request }) => {
		const banner = page.getByRole("alert");
		const openProfile = page.getByRole("link", { name: "Open Profile" });
		const press = page.getByRole("button", { name: "Change email" });

		for (const path of [
			"/auth/email",
			`/auth/email#secret=${"A".repeat(43)}`,
		]) {
			await page.goto(path);
			await expect(banner).toHaveText("Expired");
			await expect(openProfile).toHaveAttribute("href", PROFILE);
			await expect(press).toHaveCount(0);
		}

		const used = await signUp();
		const usedLink = await askForEmail(used, `new-${used.user.email}`);
		const usedAsking = await asking(newContext, used);
		await usedAsking.goto(usedLink.href);
		await usedAsking.getByRole("button", { name: "Change email" }).click();
		await usedAsking.waitForURL(settings(used.user.slug));
		await page.goto(usedLink.href);
		await expect(page.getByRole("heading", { level: 1 })).toHaveText(
			"Change email",
		);
		await expect(banner).toHaveText("Already used");
		await expect(openProfile).toHaveAttribute("href", PROFILE);
		await expect(press).toHaveCount(0);

		const taken = await signUp();
		const takenEmail = `new-${taken.user.email}`;
		const takenLink = await askForEmail(taken, takenEmail);
		await signUp({ email: takenEmail });
		const takenAsking = await asking(newContext, taken);
		const takenBanner = takenAsking.getByRole("alert");
		const takenPress = takenAsking.getByRole("button", {
			name: "Change email",
		});
		await takenAsking.goto(takenLink.href);
		await takenPress.click();
		await expect(takenBanner).toHaveText("Address taken");
		await expect(takenAsking.locator(".kv")).toHaveText([`to${takenEmail}`]);
		await expect(takenPress).toHaveCount(0);
		expect(await apiEmail(request, taken)).toBe(taken.user.email);

		const elsewhere = await signUp();
		await page.goto(
			(await askForEmail(elsewhere, `new-${elsewhere.user.email}`)).href,
		);
		await press.click();
		await expect(banner).toHaveText(ELSEWHERE);
		await expect(openProfile).toHaveAttribute("href", PROFILE);
		await expect(press).toHaveCount(0);
	});

	// Kills a state of board 05 that fails axe in either theme.
	test("each state passes axe in both themes", async ({ page, newContext }) => {
		const reader = await signUp();
		const link = await askForEmail(reader, `new-${reader.user.email}`);
		// Each scan opens the link afresh, as a new page: the page took the secret
		// out of its address, and a new fragment alone would not load one.
		await axeInBothThemes(
			page,
			afresh(link),
			() =>
				expect(
					page.getByRole("button", { name: "Change email" }),
				).toBeVisible(),
			async () => {},
		);
		await axeInBothThemes(page, "/auth/email", () =>
			expect(page.getByRole("alert")).toHaveText("Expired"),
		);

		await axeInBothThemes(
			page,
			afresh(link),
			() => expect(page.getByRole("alert")).toHaveText(ELSEWHERE),
			async () => {
				await page.getByRole("button", { name: "Change email" }).click();
			},
		);

		const taken = await signUp();
		const takenEmail = `new-${taken.user.email}`;
		const takenLink = await askForEmail(taken, takenEmail);
		await signUp({ email: takenEmail });
		const takenAsking = await asking(newContext, taken);
		await axeInBothThemes(
			takenAsking,
			afresh(takenLink),
			() => expect(takenAsking.getByRole("alert")).toHaveText("Address taken"),
			async () => {
				await takenAsking.getByRole("button", { name: "Change email" }).click();
			},
		);

		const readerAsking = await asking(newContext, reader);
		await readerAsking.goto(link.href);
		await readerAsking.getByRole("button", { name: "Change email" }).click();
		await readerAsking.waitForURL(settings(reader.user.slug));
		await axeInBothThemes(
			page,
			afresh(link),
			() => expect(page.getByRole("alert")).toHaveText("Already used"),
			async () => {},
		);
	});
});

/** A page in the browser of the session `reader` asks for an email change in. */
const asking = async (
	newContext: (options?: BrowserContextOptions) => Promise<BrowserContext>,
	reader: Reader,
) => (await newContext({ storageState: signedIn(reader) })).newPage();

/** Ask for `newEmail` in the reader's session, as only a session may, and return the confirm link it sends. */
const askForEmail = async (reader: Reader, newEmail: string) => {
	const asked = await inSession(
		reader,
		"PATCH",
		`/v0/users/${reader.user.uuid}`,
		{ email: newEmail },
	);
	expect(asked.status).toBe(200);
	return mailedLink(newEmail, CONFIRM);
};

/** The link with a query the page ignores, so opening it loads the page anew. */
const afresh = (link: URL) => `${link.pathname}?axe${link.hash}`;

/** Every confirm the page sends from now on, through any redirect. */
const sentConfirms = (page: Page) => {
	const sent: string[] = [];
	page.on("request", (request) => {
		if (request.url().endsWith("/v0/auth/email/change/confirm")) {
			sent.push(request.url());
		}
	});
	return sent;
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
