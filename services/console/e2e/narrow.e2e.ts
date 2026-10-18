import {
	crumb,
	expect,
	nextPath,
	seed,
	signedIn,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;

test.use({
	storageState: signedIn(seed.member),
	viewport: { width: 390, height: 844 },
});

// Kills a narrow bar that shows the project in place of Bencher, drops the
// bell, Feedback, the menu, or the avatar, or keeps Docs out of the menu; a tab
// row left scrolled to its start; and touch targets under 44 px.
test("the narrow bar keeps the wordmark, the bell, Feedback, the menu, and the avatar, and the active tab is in view", async ({
	page,
}) => {
	await page.goto(nextPath(hashbrown.slug, "settings"));
	const bar = page.getByRole("banner");

	const mark = bar.getByRole("link", { name: "Bencher, home" });
	const bell = bar.getByRole("link", { name: "1 active alert" });
	const feedback = bar.getByRole("button", { name: "Feedback", exact: true });
	const menu = bar.getByRole("button", { name: "Menu" });
	const avatar = bar.getByRole("link", { name: "Account" });
	for (const control of [mark, bell, feedback, menu, avatar]) {
		await expect(control).toBeVisible();
	}
	await expect(crumb(page)).toBeHidden();
	await expect(bar.getByRole("link", { name: "Docs" })).toBeHidden();

	const settings = tabRow(page).getByRole("link", { name: "Settings" });
	await expect(settings).toHaveAttribute("aria-current", "page");
	const box = await settings.boundingBox();
	expect(box?.x).toBeGreaterThanOrEqual(0);
	expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(390);

	const tabs = await tabRow(page).getByRole("link").all();
	for (const target of [mark, bell, feedback, menu, avatar, ...tabs]) {
		const size = await target.boundingBox();
		expect(size?.height).toBeGreaterThanOrEqual(44);
		expect(size?.width).toBeGreaterThanOrEqual(44);
	}

	await menu.click();
	await expect(bar.getByRole("link", { name: "Docs" })).toBeVisible();
	await expect(
		bar.getByRole("button", { name: "Switch between light and dark" }),
	).toBeVisible();
});
