import type { APIRequestContext } from "@playwright/test";
import type { JsonUser } from "../src/types/bencher";
import { axeInBothThemes } from "./axe";
import { type Reader, expect, seed, signedIn, test } from "./fixtures";
import { mailed } from "./mail";
import { inSession, signInByEmail, signUp } from "./session";

const USERS = "/console/server/users";
const CHANGED = "Bencher Email Changed";
const settings = (slug: string) => `/console/users/${slug}/settings`;

test.describe("a server admin", () => {
	test.use({ storageState: signedIn(seed.admin) });

	// Kills a Users section missing from a server admin's account, a search
	// that does not find a user, a row that does not open their Profile,
	// switches that do not lock, unlock, or make an admin, a lock that leaves
	// their sessions, and an email change that waits for a link, keeps a
	// session, or tells either address nothing.
	test("finds a user, locks and unlocks them, makes them an admin, and changes their email", async ({
		page,
		request,
	}) => {
		const user = await signUp();
		await page.goto(settings(seed.admin.user.slug));
		await page
			.getByRole("navigation", { name: "Account" })
			.or(page.getByRole("navigation", { name: "Account sections" }))
			.getByRole("link", { name: "Users" })
			.first()
			.click();
		await expect(page).toHaveURL(USERS);
		await page
			.getByRole("searchbox", { name: "Search users" })
			.fill(user.user.name);
		const table = page.getByRole("table", { name: "Users" });
		await expect(table.getByRole("row")).toHaveCount(2);
		await table.getByRole("link", { name: user.user.name }).click();
		await expect(page).toHaveURL(settings(user.user.slug));

		const locked = page.getByRole("button", { name: "Locked", exact: true });
		const admin = page.getByRole("button", { name: "Admin", exact: true });
		await expect(locked).toHaveAttribute("aria-pressed", "false");
		await locked.click();
		await expect(locked).toHaveAttribute("aria-pressed", "true");
		expect((await apiUser(request, user)).locked).toBe(true);
		expect((await inSession(user, "GET", "/v0/auth/session")).status).toBe(401);
		await locked.click();
		await expect(locked).toHaveAttribute("aria-pressed", "false");
		expect((await apiUser(request, user)).locked).toBe(false);

		await admin.click();
		await expect(admin).toHaveAttribute("aria-pressed", "true");
		expect((await apiUser(request, user)).admin).toBe(true);
		await admin.click();
		await expect(admin).toHaveAttribute("aria-pressed", "false");
		expect((await apiUser(request, user)).admin).toBe(false);

		const signedInAgain = await signInByEmail(user.user.email);
		const newEmail = `new-${user.user.email}`;
		await page.getByRole("textbox", { name: "Email" }).fill(newEmail);
		await page.getByRole("button", { name: "Save changes" }).click();
		const dialog = page.getByRole("alertdialog");
		await expect(dialog).toContainText(
			`Change ${user.user.name}'s email to ${newEmail}?`,
		);
		await expect(dialog).toContainText(
			`Every session of ${user.user.name} ends`,
		);
		await dialog.getByRole("button", { name: "Change email" }).click();
		await expect(
			page.getByRole("status").filter({ hasText: "Saved" }),
		).toHaveText("Saved");
		expect((await apiUser(request, user)).email).toBe(newEmail);
		for (const to of [user.user.email, newEmail]) {
			expect(await mailed(to, CHANGED)).toContain(user.user.name);
		}
		expect(
			(await inSession(signedInAgain, "GET", "/v0/auth/session")).status,
		).toBe(401);
	});

	// Kills a Users page or a server admin's switches that fail axe in either theme.
	test("Users and the switches pass axe in both themes", async ({ page }) => {
		const user = await signUp();
		await axeInBothThemes(page, USERS, () =>
			expect(
				page.getByRole("table", { name: "Users" }).getByRole("row").nth(1),
			).toBeVisible(),
		);
		await axeInBothThemes(page, settings(user.user.slug), () =>
			expect(
				page.getByRole("button", { name: "Locked", exact: true }),
			).toBeVisible(),
		);
	});
});

test.describe("a reader who is not a server admin", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a Users section or a switch shown to a reader the API would refuse,
	// and a Users page that lists anything for them.
	test("sees no Users page and no switches", async ({ page }) => {
		await page.goto(settings(seed.member.user.slug));
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
			seed.member.user.email,
		);
		await expect(page.getByRole("link", { name: "Users" })).toHaveCount(0);
		await expect(
			page.getByRole("button", { name: "Admin", exact: true }),
		).toHaveCount(0);
		await expect(
			page.getByRole("button", { name: "Locked", exact: true }),
		).toHaveCount(0);

		await page.goto(USERS);
		await expect(page.getByRole("alert")).toHaveText(/Users did not load/);
		await expect(page.getByRole("table", { name: "Users" })).toHaveCount(0);
	});
});

/** The user as the API holds them, read as the server admin. */
const apiUser = async (request: APIRequestContext, reader: Reader) => {
	const response = await request.get(
		`${seed.api_url}/v0/users/${reader.user.uuid}`,
		{ headers: { Authorization: `Bearer ${seed.admin.key}` } },
	);
	expect(response.status()).toBe(200);
	return (await response.json()) as JsonUser;
};
