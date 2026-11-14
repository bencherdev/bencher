import type {
	BrowserContext,
	BrowserContextOptions,
	Page,
} from "@playwright/test";
import type { JsonOrganization } from "../src/types/bencher";
import { createOwnProject, unique } from "./api";
import { axeInBothThemes } from "./axe";
import { typeCode } from "./door";
import {
	RELAY,
	type Reader,
	consolePath,
	expect,
	provedAgo,
	seed,
	signedIn,
	test,
} from "./fixtures";
import { mailMark, mailedSignIn } from "./mail";
import { inSession, signUp } from "./session";

const HOUR = 60 * 60;

type NewContext = (options?: BrowserContextOptions) => Promise<BrowserContext>;

/**
 * A reader of the test's own who signed in an hour ago, past the 10 minutes a
 * key or a delete takes, and a browser signed in as them.
 */
const stale = async (newContext: NewContext) => {
	const reader = await signUp();
	provedAgo(reader.session, HOUR);
	const context = await newContext({ storageState: signedIn(reader) });
	return { reader, context, page: await context.newPage() };
};

const keysPath = (reader: Reader) => `/console/users/${reader.user.slug}/keys`;
const again = (page: Page) =>
	page.getByRole("dialog", { name: "Sign in again" });
const reveal = (page: Page) =>
	page.getByRole("dialog", { name: "Copy your new key" });

/** Ask for a key named `name` on the reader's Keys page, as far as the press of Create key. */
const createKey = async (page: Page, reader: Reader, name: string) => {
	await page.goto(keysPath(reader));
	await page.getByRole("button", { name: "New key" }).click();
	const form = page.getByRole("dialog", { name: "New key" });
	await form.getByRole("textbox", { name: "Name" }).fill(name);
	await form.getByRole("button", { name: "Create key" }).click();
};

test.describe("Sign in again", () => {
	// Kills a refusal for an old sign-in that shows only an error, a dialog
	// that offers no email, a code that proves nothing, and a key that needs
	// a second press of Create key once the reader signed in again.
	test("a new key asks to sign in again, and the code makes the key", async ({
		newContext,
	}) => {
		const { reader, page } = await stale(newContext);
		const name = unique("fresh-key");
		const mark = mailMark();
		await createKey(page, reader, name);

		await again(page).getByRole("button", { name: "Email a code" }).click();
		await expect(again(page).getByText(reader.user.email)).toBeVisible();
		const { code } = await mailedSignIn(reader.user.email, mark);
		await typeCode(page, code);
		await expect(again(page)).toBeHidden();
		await expect(reveal(page)).toContainText(name);
		await reveal(page).getByRole("button", { name: "Done" }).click();
		await expect(
			page
				.getByRole("table", { name: "Active keys" })
				.getByRole("row", { name: new RegExp(name) }),
		).toHaveCount(1);
	});

	// Kills a link that signs in anew, or that the asking tab never hears, so
	// its dialog waits for a code the reader will not type, and a used link
	// that sends a signed in reader to the door's Use another email.
	test("the link in another tab moves the dialog on, then ends", async ({
		newContext,
	}) => {
		const { reader, context, page } = await stale(newContext);
		const name = unique("linked-key");
		const mark = mailMark();
		await createKey(page, reader, name);
		await again(page).getByRole("button", { name: "Email a code" }).click();
		const { link } = await mailedSignIn(reader.user.email, mark);

		const tab = await context.newPage();
		await tab.goto(link.toString());
		await expect(tab).toHaveURL(/\/console\//);
		await expect(again(page)).toBeHidden();
		await expect(reveal(page)).toContainText(name);
		const cookies = await context.cookies();
		expect(cookies.filter(({ name }) => name.includes("session"))).toEqual([
			expect.objectContaining({ value: reader.session }),
		]);

		const used = await context.newPage();
		await used.goto(link.toString());
		await expect(used.getByRole("alert")).toHaveText("Expired");
		await expect(
			used.getByRole("link", { name: "Back to the console" }),
		).toHaveAttribute("href", /^\/console/);
		await expect(
			used.getByRole("button", { name: "Use another email" }),
		).toHaveCount(0);
	});

	// Kills a link that proves a session in a browser that never asked, or
	// that spends itself there, so the asking browser loses it.
	test("the link in another browser says where to open it, and proves nothing", async ({
		newContext,
	}) => {
		const { reader, page } = await stale(newContext);
		const name = unique("elsewhere-key");
		const mark = mailMark();
		await createKey(page, reader, name);
		await again(page).getByRole("button", { name: "Email a code" }).click();
		const { code, link } = await mailedSignIn(reader.user.email, mark);

		const other = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		await other.goto(link.toString());
		await expect(other.getByRole("alert")).toHaveText(
			"Open this link on the device where you were asked",
		);
		await expect(other).toHaveURL(/\/auth\/confirm/);
		await expect(again(page)).toBeVisible();

		await typeCode(page, code);
		await expect(reveal(page)).toContainText(name);
	});

	// Kills a provider that leaves this tab, and the reader's half made key,
	// rather than signing in again in a window of its own, and a dialog that
	// never hears its proof.
	test("a provider pinned to the reader signs in again in its own window", async ({
		newContext,
	}) => {
		const { reader, context, page } = await stale(newContext);
		const state = "fresh-state";
		const started: string[] = [];
		await context.route(`${RELAY}/v0/auth/fresh`, (route) =>
			route.fulfill({ json: { providers: ["github"] } }),
		);
		await context.route(
			`${RELAY}/v0/auth/fresh/providers/github/start`,
			(route) => {
				started.push(route.request().method());
				return route.fulfill({
					json: {
						url: `https://github.example/login/oauth/authorize?state=${state}`,
					},
				});
			},
		);
		await context.route("https://github.example/**", (route) =>
			route.fulfill({ contentType: "text/plain", body: "GitHub" }),
		);
		// The suite's API signs in with no GitHub, so this stands in for its proof.
		await context.route(`${RELAY}/v0/auth/providers/github/finish`, (route) => {
			provedAgo(reader.session, 0);
			return route.fulfill({ json: { proved: new Date().toISOString() } });
		});
		const name = unique("github-key");
		await createKey(page, reader, name);

		const opened = page.waitForEvent("popup");
		await again(page)
			.getByRole("button", { name: "Continue with GitHub" })
			.click();
		const popup = await opened;
		await expect(popup).toHaveURL(/^https:\/\/github\.example\//);
		expect(started).toEqual(["POST"]);
		await expect(page).toHaveURL(new RegExp(`${keysPath(reader)}$`));

		const closed = popup.waitForEvent("close");
		await popup.goto(
			`/auth/github?${new URLSearchParams({ code: "c", state })}`,
		);
		await closed;
		await expect(again(page)).toBeHidden();
		await expect(reveal(page)).toContainText(name);
	});

	// Kills a Cancel that makes the key anyway, or that loses what the reader
	// typed and why nothing was made, or cuts the reason short on a phone.
	test("Cancel keeps the form and says why the key was not made", async ({
		newContext,
	}) => {
		const { reader, page } = await stale(newContext);
		await page.setViewportSize({ width: 390, height: 844 });
		const name = unique("cancelled-key");
		await createKey(page, reader, name);
		await again(page).getByRole("button", { name: "Cancel" }).click();
		const form = page.getByRole("dialog", { name: "New key" });
		const reason = form.getByRole("alert");
		await expect(reason).toHaveText(
			"Bencher did not make the key: This needs a browser session signed in within the last 10 minutes",
		);
		// Every line of the reason ends inside its box, so none of it is cut.
		expect(
			await reason.evaluate((element) => {
				const range = document.createRange();
				range.selectNodeContents(element);
				const box = element.getBoundingClientRect();
				return [...range.getClientRects()].every(
					(line) => line.right <= box.right + 0.5,
				);
			}),
		).toBe(true);
		await expect(form.getByRole("textbox", { name: "Name" })).toHaveValue(name);
		await form.getByRole("button", { name: "Cancel" }).click();
		await expect(
			page
				.getByRole("table", { name: "Active keys" })
				.getByRole("row", { name: new RegExp(name) }),
		).toHaveCount(0);
	});

	// Kills a delete that needs no fresh sign-in, and one that the dialog's
	// proof leaves undone.
	test("deleting a project asks to sign in again, then deletes it", async ({
		newContext,
		request,
	}) => {
		const { reader, page } = await stale(newContext);
		const project = await createOwnProject(request, reader);
		const mark = mailMark();
		await page.goto(consolePath(project.slug, "settings"));
		await page.getByRole("button", { name: "Delete project" }).click();
		const dialog = page.getByRole("alertdialog", {
			name: `Delete ${project.slug}?`,
		});
		await dialog
			.getByRole("textbox", { name: `Type ${project.slug} to confirm` })
			.fill(project.slug);
		await dialog.getByRole("button", { name: "Delete project" }).click();

		await again(page).getByRole("button", { name: "Email a code" }).click();
		const { code } = await mailedSignIn(reader.user.email, mark);
		await typeCode(page, code);
		await expect(page).not.toHaveURL(/\/settings$/);
		const read = await request.get(
			`${seed.api_url}/v0/projects/${project.slug}`,
			{ headers: { Authorization: `Bearer ${reader.key}` } },
		);
		expect(read.status()).toBe(404);
	});

	// Kills an invitation, which grants Leader, that a sign-in more than 10
	// minutes old sends, and one that the dialog's proof leaves unsent.
	test("an invite asks to sign in again, then sends", async ({
		newContext,
	}) => {
		const { reader, page } = await stale(newContext);
		const [own] = (
			await inSession<JsonOrganization[]>(reader, "GET", "/v0/organizations")
		).data;
		const email = `${unique("courage").replace(" ", "-")}@nowhere.example`;
		const mark = mailMark();
		await page.goto(`/console/organizations/${own?.slug}/members`);
		await page.getByRole("textbox", { name: "Email" }).fill(email);
		await page.getByRole("button", { name: "Send invite" }).click();

		await again(page).getByRole("button", { name: "Email a code" }).click();
		const { code } = await mailedSignIn(reader.user.email, mark);
		await typeCode(page, code);
		await expect(page.getByRole("main").getByRole("status")).toHaveText(
			`Invitation sent to ${email}`,
		);
		await expect(
			page
				.getByRole("table", { name: "Pending invites" })
				.getByRole("row", { name: new RegExp(email) }),
		).toHaveCount(1);
	});

	// Kills a dialog, or its code step, that fails axe in either theme.
	test("Sign in again passes axe in both themes", async ({ newContext }) => {
		const { reader, page } = await stale(newContext);
		await axeInBothThemes(
			page,
			keysPath(reader),
			async () => {
				await expect(
					again(page).getByRole("textbox", { name: "Digit 1" }),
				).toBeVisible();
			},
			async () => {
				await createKey(page, reader, unique("axe-key"));
				await again(page).getByRole("button", { name: "Email a code" }).click();
			},
		);
	});
});
