import {
	THEME_KEY,
	crumb,
	expect,
	consolePath,
	seed,
	signedIn,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;

test.use({ storageState: signedIn(seed.member), colorScheme: "dark" });

// Kills a toggle that does not store the theme, a theme read back from
// anywhere but the key the classic pages read, and a page that paints before it.
test("the theme toggle sets the theme, keeps it across a reload, and shares it with the classic pages", async ({
	page,
}) => {
	const html = page.locator("html");
	await page.goto(consolePath(hashbrown.slug, "runs"));
	await expect(html).toHaveAttribute("data-theme", "dark");
	// The toggle works once the app has drawn over the first paint.
	await expect(
		crumb(page, `${seed.organization.name} / ${hashbrown.name}`),
	).toBeVisible();

	await page
		.getByRole("button", { name: "Switch between light and dark" })
		.click();
	await expect(html).toHaveAttribute("data-theme", "light");
	expect(
		await page.evaluate((key) => localStorage.getItem(key), THEME_KEY),
	).toBe("light");

	await page.reload();
	await expect(html).toHaveAttribute("data-theme", "light");

	await page.goto(`/console/users/${seed.member.user.slug}/help`);
	await expect(html).toHaveAttribute("data-theme", "light");
});

test.describe("with the system set to light", () => {
	test.use({ colorScheme: "light" });

	// Kills a wordmark that keeps to the system once the reader picks a theme,
	// or to the theme the page started with once it is switched.
	test("the wordmark follows the system until the reader picks a theme", async ({
		page,
	}) => {
		const html = page.locator("html");
		const wordmark = page.getByRole("banner").locator(".brand img:visible");
		await page.goto(consolePath(hashbrown.slug, "runs"));
		await expect(html).toHaveAttribute("data-theme", "light");
		await expect(wordmark).toHaveAttribute("src", "/wordmark-light.svg");
		await expect(
			crumb(page, `${seed.organization.name} / ${hashbrown.name}`),
		).toBeVisible();

		await page
			.getByRole("button", { name: "Switch between light and dark" })
			.click();
		await expect(wordmark).toHaveAttribute("src", "/wordmark-dark.svg");
		await page.reload();
		await expect(html).toHaveAttribute("data-theme", "dark");
		await expect(wordmark).toHaveAttribute("src", "/wordmark-dark.svg");
	});
});
