import {
	crumbs,
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

// Kills a narrow bar that drops the project, the bell, or the avatar, or keeps
// what it should fold away; a tab row left scrolled to its start; and touch
// targets under 44 px.
test("the narrow bar keeps the mark, the project, the bell, and the avatar, and the active tab is in view", async ({
	page,
}) => {
	await page.goto(nextPath(hashbrown.slug, "settings"));
	const bar = page.getByRole("banner");

	const mark = bar.getByRole("link", { name: "Bencher, home" });
	const project = crumbs(page).getByRole("link", { name: hashbrown.name });
	const bell = bar.getByRole("link", { name: "Alerts, 1 active" });
	const avatar = bar.getByRole("link", { name: "Account" });
	for (const control of [mark, project, bell, avatar]) {
		await expect(control).toBeVisible();
	}
	await expect(
		crumbs(page).getByRole("link", { name: seed.organization.name }),
	).toBeHidden();
	await expect(bar.getByRole("link", { name: "Docs" })).toBeHidden();

	const settings = tabRow(page).getByRole("link", { name: "Settings" });
	await expect(settings).toHaveAttribute("aria-current", "page");
	const box = await settings.boundingBox();
	expect(box?.x).toBeGreaterThanOrEqual(0);
	expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(390);

	const tabs = await tabRow(page).getByRole("link").all();
	for (const target of [project, bell, avatar, ...tabs]) {
		const size = await target.boundingBox();
		expect(size?.height).toBeGreaterThanOrEqual(44);
		expect(size?.width).toBeGreaterThanOrEqual(44);
	}
});
