import type { Page } from "@playwright/test";
import {
	createKey,
	createOrganization,
	createProject,
	projectStatus,
	unique,
} from "./api";
import { axeInBothThemes } from "./axe";
import {
	VERSION_KEY,
	crumbs,
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;

const READ_ONLY = "Read only. Ask a project Maintainer for access.";

const general = (page: Page) =>
	page.getByRole("heading", { level: 1, name: "General" });
const keys = (page: Page) =>
	page.getByRole("heading", { level: 1, name: "Keys" });
const save = (page: Page) => page.getByRole("button", { name: "Save changes" });
/** The page's own status line; the shell has one for page changes. */
const status = (page: Page) => page.getByRole("main").getByRole("status");

/** Hold every request matching `method` and `url` until the returned release is called. */
const hold = async (page: Page, method: string, url: string) => {
	let release = () => {};
	const released = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route(url, async (route) => {
		if (route.request().method() === method) {
			await released;
		}
		await route.fallback();
	});
	return release;
};

test.describe("General, as the project's Maintainer", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a rename that waits for the API before the crumb changes, a save
	// that never reaches the API, and a form that reads a stale project.
	test("a rename shows in the crumb at once and survives a reload", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		const renamed = unique("Hash Browns");
		await page.goto(nextPath(project.slug, "settings"));
		await expect(general(page)).toBeVisible();
		await expect(save(page)).toBeDisabled();

		const release = await hold(
			page,
			"PATCH",
			`${seed.api_url}/v0/projects/${project.slug}`,
		);
		await page.getByRole("textbox", { name: "Name" }).fill(renamed);
		await save(page).click();
		await expect(
			crumbs(page).getByRole("link", { name: renamed }),
		).toBeVisible();
		release();
		await expect(status(page)).toHaveText("Saved");
		await settle(page);

		await page.reload();
		await expect(
			crumbs(page).getByRole("link", { name: renamed }),
		).toBeVisible();
		await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			renamed,
		);
	});

	// Kills a slug change that leaves the page on a URL that no longer names a
	// project, and a version memory left under the old slug.
	test("a slug change moves the page to the new slug and the version memory follows", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		const slug = `${project.slug}-moved`;
		await page.goto(nextPath(project.slug, "settings"));
		await expect(general(page)).toBeVisible();

		await page.getByRole("textbox", { name: "Slug" }).fill(slug);
		await save(page).click();
		await expect(page).toHaveURL(nextPath(slug, "settings"));
		const versions = await page.evaluate(
			(key) => JSON.parse(localStorage.getItem(key) ?? "{}"),
			VERSION_KEY,
		);
		expect(versions[slug]).toBe(1);
		expect(versions[project.slug]).toBeUndefined();

		await page.reload();
		await expect(general(page)).toBeVisible();
		await expect(page.getByRole("textbox", { name: "Slug" })).toHaveValue(slug);
		expect(await projectStatus(request, project.slug)).toBe(404);
	});

	// Kills a refusal that is swallowed, shown far from the field, or left
	// applied in the cache as if the project were private.
	test("private on an organization with no plan is refused in place, with the plans link", async ({
		page,
		request,
	}) => {
		const organization = await createOrganization(request);
		const project = await createProject(request, {
			organization: organization.slug,
		});
		await page.goto(nextPath(project.slug, "settings"));
		await expect(general(page)).toBeVisible();

		await page.getByRole("radio", { name: "Private" }).check();
		await save(page).click();
		const refusal = page.getByRole("alert");
		await expect(refusal).toContainText("private needs a paid plan");
		await expect(
			refusal.getByRole("link", { name: "See plans" }),
		).toHaveAttribute(
			"href",
			`/console/organizations/${organization.slug}/billing`,
		);

		await page.reload();
		await expect(page.getByRole("radio", { name: "Public" })).toBeChecked();
	});

	// Kills a Danger section drawn from the wrong permission, and a delete that
	// is not held until the slug is typed exactly.
	test("Delete stays off until the slug matches, then deletes and lands on the organization's projects", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await page.goto(nextPath(project.slug, "settings"));
		await expect(
			page.getByRole("heading", { level: 2, name: "Danger" }),
		).toBeVisible();

		await page.getByRole("button", { name: "Delete project" }).click();
		const dialog = page.getByRole("alertdialog", {
			name: `Delete ${project.slug}?`,
		});
		await expect(dialog).toBeVisible();
		const confirm = dialog.getByRole("button", { name: "Delete project" });
		const typed = dialog.getByRole("textbox", {
			name: `Type ${project.slug} to confirm`,
		});
		await expect(confirm).toBeDisabled();
		await typed.fill(project.slug.slice(0, -1));
		await expect(confirm).toBeDisabled();
		await typed.fill(`${project.slug} `);
		await expect(confirm).toBeDisabled();
		await typed.fill(project.slug);
		await expect(confirm).toBeEnabled();

		await confirm.click();
		await expect(page).toHaveURL(
			`/console/organizations/${seed.organization.uuid}/projects`,
		);
		expect(await projectStatus(request, project.slug)).toBe(404);
	});

	// Kills a delete that leaves the app before the cache has written the
	// project's removal, so the browser's store keeps the deleted project.
	test("a deleted project leaves the browser's store", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await page.goto(nextPath(project.slug, "settings"));
		await expect(
			page.getByRole("heading", { level: 2, name: "Danger" }),
		).toBeVisible();
		await expect
			.poll(() => storedQueries(page, project.slug), { timeout: 5_000 })
			.not.toEqual([]);

		await page.getByRole("button", { name: "Delete project" }).click();
		const dialog = page.getByRole("alertdialog", {
			name: `Delete ${project.slug}?`,
		});
		await dialog
			.getByRole("textbox", { name: `Type ${project.slug} to confirm` })
			.fill(project.slug);
		await dialog.getByRole("button", { name: "Delete project" }).click();
		await expect(page).toHaveURL(
			`/console/organizations/${seed.organization.uuid}/projects`,
		);
		expect(await storedQueries(page, project.slug)).toEqual([]);
	});

	// Kills a Cancel that deletes, or that leaves the dialog in the way.
	test("Cancel closes the delete dialog and keeps the project", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await page.goto(nextPath(project.slug, "settings"));
		await page.getByRole("button", { name: "Delete project" }).click();
		const dialog = page.getByRole("alertdialog");
		await dialog
			.getByRole("textbox", { name: `Type ${project.slug} to confirm` })
			.fill(project.slug);
		await dialog.getByRole("button", { name: "Cancel" }).click();
		await expect(dialog).toBeHidden();
		expect(await projectStatus(request, project.slug)).toBe(200);
	});
});

test.describe("as a reader who is a member of nothing", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	// Kills controls drawn for a reader the API would refuse: the form, Save,
	// and the Danger section.
	test("General is read only, with no Danger section", async ({ page }) => {
		await page.goto(nextPath(hashbrown.slug, "settings"));
		await expect(general(page)).toBeVisible();
		await expect(page.getByText(READ_ONLY)).toBeVisible();
		await expect(page.getByText(hashbrown.slug, { exact: true })).toBeVisible();

		await expect(page.getByRole("textbox")).toHaveCount(0);
		await expect(page.getByRole("radio")).toHaveCount(0);
		await expect(save(page)).toHaveCount(0);
		await expect(page.getByRole("heading", { name: "Danger" })).toHaveCount(0);
		await expect(
			page.getByRole("button", { name: "Delete project" }),
		).toHaveCount(0);
	});

	// Kills key controls, or a key list, shown to a reader without `manage`.
	test("Keys shows no key and no control", async ({ page }) => {
		await page.goto(nextPath(hashbrown.slug, "settings/keys"));
		await expect(keys(page)).toBeVisible();
		await expect(page.getByText(READ_ONLY)).toBeVisible();
		await expect(page.getByRole("button", { name: "New key" })).toHaveCount(0);
		await expect(page.getByRole("table")).toHaveCount(0);
	});
});

test.describe("Keys, as the project's Maintainer", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a reveal that can be shown twice, a secret kept anywhere the browser
	// stores, and a new key missing from the list.
	test("a new key is shown once, and is gone after navigation", async ({
		page,
	}) => {
		const name = unique("release-benchmarks");
		await page.goto(nextPath(hashbrown.slug, "settings/keys"));
		await expect(keys(page)).toBeVisible();

		await page.getByRole("button", { name: "New key" }).click();
		const form = page.getByRole("dialog", { name: "New key" });
		await expect(
			form.getByRole("button", { name: "Create key" }),
		).toBeDisabled();
		await form.getByRole("textbox", { name: "Name" }).fill(name);
		await form.getByRole("radio", { name: /^30 days/ }).check();
		await form.getByRole("button", { name: "Create key" }).click();

		const reveal = page.getByRole("dialog", { name: "Copy your new key" });
		await expect(reveal).toContainText(name);
		const secret = (
			await reveal.getByText(/^bencher_run_\w+$/).textContent()
		)?.trim();
		expect(secret).toMatch(/^bencher_run_\w+$/);
		await expect(reveal).toContainText(`export BENCHER_API_KEY=${secret}`);
		await reveal.getByRole("button", { name: "Done" }).click();
		await expect(reveal).toBeHidden();

		const active = page.getByRole("table", { name: "Active keys" });
		await expect(
			active.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();
		await expect(page.getByText(secret ?? "")).toHaveCount(0);

		await page.getByRole("link", { name: "General" }).first().click();
		await expect(general(page)).toBeVisible();
		await page.getByRole("link", { name: "Keys" }).first().click();
		await expect(
			active.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();
		await expect(page.getByText(secret ?? "")).toHaveCount(0);

		// The cache persists to IndexedDB a moment after it changes.
		await settle(page, 1_500);
		const stored = await page.evaluate(async () => {
			const local = JSON.stringify({ ...localStorage });
			const cache = await new Promise<string>((resolve) => {
				const open = indexedDB.open("bencher-console");
				open.onsuccess = () => {
					const read = open.result
						.transaction("cache")
						.objectStore("cache")
						.getAll();
					read.onsuccess = () => resolve(JSON.stringify(read.result));
					read.onerror = () => resolve("");
				};
				open.onerror = () => resolve("");
			});
			return local + cache;
		});
		expect(stored).toContain(name);
		expect(stored).not.toContain(secret);
	});

	// Kills a revoke without its confirmation, one that only hides the row, and
	// a Revoked list that does not hold it.
	test("Revoke confirms that it cannot be undone and moves the key to Revoked", async ({
		page,
		request,
	}) => {
		const name = unique("pr-preview");
		await createKey(request, hashbrown.slug, name);
		await page.goto(nextPath(hashbrown.slug, "settings/keys"));
		const active = page.getByRole("table", { name: "Active keys" });
		await expect(
			active.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();

		await page.getByRole("button", { name: `Revoke ${name}` }).click();
		const confirm = page.getByRole("alertdialog", { name: `Revoke ${name}?` });
		await expect(confirm).toContainText("Revoking cannot be undone");
		await confirm.getByRole("button", { name: "Revoke key" }).click();
		await expect(confirm).toBeHidden();
		await expect(status(page)).toHaveText(
			`Revoked ${name}. It cannot be used again.`,
		);
		await expect(
			active.getByRole("row", { name: new RegExp(name) }),
		).toHaveCount(0);

		await page.getByRole("radio", { name: /^Revoked/ }).check();
		const revoked = page.getByRole("table", { name: "Revoked keys" });
		await expect(
			revoked.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();
		await expect(
			page.getByRole("button", { name: `Revoke ${name}` }),
		).toHaveCount(0);

		await settle(page);
		await page.reload();
		await page.getByRole("radio", { name: /^Revoked/ }).check();
		await expect(
			revoked.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();
	});
});

test.describe("accessibility", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills contrast, names, and roles that fail axe in either theme.
	test("General and its delete dialog pass axe in both themes", async ({
		page,
	}) => {
		await axeInBothThemes(
			page,
			nextPath(hashbrown.slug, "settings"),
			async () => {
				await expect(general(page)).toBeVisible();
				await page.getByRole("button", { name: "Delete project" }).click();
				await expect(page.getByRole("alertdialog")).toBeVisible();
			},
		);
	});

	test("Keys and its new key dialog pass axe in both themes", async ({
		page,
	}) => {
		await axeInBothThemes(
			page,
			nextPath(hashbrown.slug, "settings/keys"),
			async () => {
				await expect(
					page.getByRole("table", { name: "Active keys" }),
				).toBeVisible();
				await page.getByRole("button", { name: "New key" }).click();
				await expect(
					page.getByRole("dialog", { name: "New key" }),
				).toBeVisible();
			},
		);
	});
});

test.describe("narrow", () => {
	test.use({
		storageState: signedIn(seed.member),
		viewport: { width: 390, height: 844 },
	});

	// Kills a rail that stays beside the form on a phone, fields that do not
	// stack, and touch targets under 44 px.
	test("the sections become a segmented control and every target is 44 px", async ({
		page,
		request,
	}) => {
		const key = unique("narrow");
		await createKey(request, hashbrown.slug, key);
		await page.goto(nextPath(hashbrown.slug, "settings"));
		await expect(general(page)).toBeVisible();
		await expect(
			page.getByRole("navigation", { name: "Settings", exact: true }),
		).toBeHidden();
		const sections = page.getByRole("navigation", {
			name: "Settings sections",
		});
		await expect(sections).toBeVisible();
		await expect(
			sections.getByRole("link", { name: "General" }),
		).toHaveAttribute("aria-current", "page");

		const name = await page
			.getByRole("textbox", { name: "Name" })
			.boundingBox();
		const label = await page.getByText("Name", { exact: true }).boundingBox();
		expect(name?.y).toBeGreaterThan((label?.y ?? 0) + (label?.height ?? 0) - 1);
		expect(name?.width).toBeGreaterThan(300);

		const targets = [
			...(await sections.getByRole("link").all()),
			page
				.getByRole("main")
				.getByRole("link", { name: "Settings", exact: true }),
			page.getByRole("textbox", { name: "Name" }),
			page.getByRole("radio", { name: "Public" }).locator(".."),
			page.getByRole("radio", { name: "Private" }).locator(".."),
			save(page),
			page.getByRole("button", { name: "Delete project" }),
		];
		for (const target of targets) {
			const box = await target.boundingBox();
			expect(box?.height).toBeGreaterThanOrEqual(44);
		}

		await sections.getByRole("link", { name: "Keys" }).click();
		await expect(keys(page)).toBeVisible();
		const revoke = page.getByRole("button", { name: `Revoke ${key}` });
		for (const target of [
			page.getByRole("button", { name: "New key" }),
			page.getByRole("radio", { name: /^Active/ }).locator(".."),
			revoke,
		]) {
			const box = await target.boundingBox();
			expect(box?.height).toBeGreaterThanOrEqual(44);
		}
	});
});

/** The query keys the browser's store holds for a project. */
const storedQueries = (page: Page, slug: string) =>
	page.evaluate(
		(slug) =>
			new Promise<string[]>((resolve) => {
				const open = indexedDB.open("bencher-console");
				open.onerror = () => resolve([]);
				open.onsuccess = () => {
					const db = open.result;
					if (!db.objectStoreNames.contains("cache")) {
						resolve([]);
						return;
					}
					const read = db.transaction("cache").objectStore("cache").getAll();
					read.onerror = () => resolve([]);
					read.onsuccess = () =>
						resolve(
							read.result.flatMap(
								(saved: { state?: { queries?: { queryKey: unknown[] }[] } }) =>
									(saved.state?.queries ?? [])
										.map(({ queryKey }) => queryKey)
										.filter((key) => key[0] === "console" && key[2] === slug)
										.map((key) => JSON.stringify(key)),
							),
						);
				};
			}),
		slug,
	);
