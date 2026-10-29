import type { Page } from "@playwright/test";
import { createOrganization, unique } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, seed, signedIn, test } from "./fixtures";

const membersPath = (organization: string) =>
	`/console/organizations/${organization}/members`;
const table = (page: Page) => page.getByRole("table", { name: "Members" });

test.use({ storageState: signedIn(seed.member) });

test.describe("Members", () => {
	// Kills an invite that never reaches the API, goes without the Leader role,
	// says nothing once sent, or lists someone who has not yet accepted.
	test("an invite reaches the API and says it was sent, adding no row", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		await page.goto(membersPath(organization.slug));
		await expect(table(page).getByRole("row")).toHaveCount(2);
		const email = `${unique("courage").replace(" ", "-")}@nowhere.example`;

		await page.getByRole("textbox", { name: "Email" }).fill(email);
		const [sent] = await Promise.all([
			page.waitForResponse(
				(response) =>
					response.request().method() === "POST" &&
					response.url() ===
						`${seed.api_url}/v0/organizations/${organization.slug}/members`,
			),
			page.getByRole("button", { name: "Send invite" }).click(),
		]);

		expect(sent.status()).toBe(202);
		expect(sent.request().postDataJSON()).toEqual({ email, role: "leader" });
		await expect(page.getByRole("main").getByRole("status")).toHaveText(
			`Invitation sent to ${email}`,
		);
		// The header row and the reader's own.
		await expect(table(page).getByRole("row")).toHaveCount(2);
	});
});

test.describe("from General", () => {
	// Kills a rail that leaves the console's app for Members, reloading the
	// page, or a title left on General.
	test("the rail opens Members without a reload", async ({ page }) => {
		await page.goto(
			`/console/organizations/${seed.organization.slug}/settings`,
		);
		await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			seed.organization.name,
		);
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});

		await page
			.getByRole("navigation", { name: "Organization settings", exact: true })
			.getByRole("link", { name: "Members" })
			.click();

		await expect(page).toHaveURL(membersPath(seed.organization.slug));
		await expect(
			table(page).getByRole("row", { name: seed.member.user.name }),
		).toBeVisible();
		await expect(page).toHaveTitle(
			`Members | ${seed.organization.name} | Bencher`,
		);
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});
});

test.describe("accessibility", () => {
	// Kills contrast, names, and roles that fail axe in either theme.
	test("Members passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(page, membersPath(seed.organization.slug), () =>
			expect(
				page.getByRole("region", { name: "Single sign-on" }),
			).toBeVisible(),
		);
	});
});
