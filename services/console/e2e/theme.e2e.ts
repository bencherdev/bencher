import { axeInBothThemes } from "./axe";
import {
	THEME_KEY,
	crumbs,
	expect,
	nextPath,
	seed,
	signedIn,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown, version_zero } = seed.projects;

test.use({ storageState: signedIn(seed.member), colorScheme: "dark" });

// Kills a toggle that does not store the theme, a theme read back from
// anywhere but the classic console's key, and a page that paints before it.
test("the theme toggle sets the theme, keeps it across a reload, and shares it with the classic console", async ({
	page,
}) => {
	const html = page.locator("html");
	await page.goto(nextPath(hashbrown.slug, "reports"));
	await expect(html).toHaveAttribute("data-theme", "dark");
	// The toggle works once the app has drawn over the first paint.
	await expect(
		crumbs(page).getByRole("link", { name: seed.organization.name }),
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

	await page.goto(`/console/projects/${version_zero.slug}/reports`);
	await expect(html).toHaveAttribute("data-theme", "light");
});

// Kills contrast, naming, and landmark failures on the shell in either theme.
test("the shell passes axe in both themes", async ({ page }) => {
	await axeInBothThemes(page, nextPath(hashbrown.slug, "reports"), async () => {
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();
	});
});
