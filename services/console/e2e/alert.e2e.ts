import { randomUUID } from "node:crypto";
import type { APIRequestContext, Page } from "@playwright/test";
import type { JsonAlert } from "../src/types/bencher";
import { createAlerts } from "./api";
import { axeInBothThemes } from "./axe";
import {
	expect,
	consolePath,
	logInTo,
	seed,
	signedIn,
	signedOut,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;

// A control names its line by what tells it apart from the lines beside it.
const BLAKE3 = "blake3";
const line = (benchmark: string) => benchmark;

const alertPage = (slug: string, alert: string) =>
	`${consolePath(slug, "alerts")}/${alert}`;
const heading = (page: Page) => page.getByRole("heading", { level: 1 });
const readout = (page: Page) => page.getByRole("figure").getByRole("status");
/** The pinned readout's link to the alert on `name`. */
const alertLink = (page: Page, name: string) =>
	readout(page).getByRole("link", { name: `Alert: ${name}` });
const badge = (page: Page, active: number) =>
	tabRow(page).getByRole("link", {
		name: active === 0 ? "Alerts" : `Alerts ${active} active`,
		exact: true,
	});
const dismiss = (scope: Page, name: string) =>
	scope.getByRole("button", { name: `Dismiss the alert on ${name}` });
const reactivate = (scope: Page, name: string) =>
	scope.getByRole("button", { name: `Reactivate the alert on ${name}` });
/** Each alert row of the Alerts list, by its name. */
const alertRows = (page: Page) =>
	page
		.getByRole("table", { name: /^Alerts/ })
		.getByRole("checkbox")
		.evaluateAll((boxes) =>
			boxes.map((box) => (box.getAttribute("aria-label") ?? "").slice(7)),
		);

/** The project's alert on `benchmark`, as the API lists it. */
const alertOf = async (
	request: APIRequestContext,
	slug: string,
	benchmark: string,
	token = seed.member.token,
) => {
	const response = await request.get(
		`${seed.api_url}/v0/projects/${slug}/alerts?per_page=255`,
		{ headers: { Authorization: `Bearer ${token}` } },
	);
	const alert = ((await response.json()) as JsonAlert[]).find(
		(each) => each.benchmark.name === benchmark && each.status === "active",
	);
	if (!alert) {
		throw new Error(`${slug} has no active alert on ${benchmark}`);
	}
	return alert;
};

/** Resolves once the console's own write of the alert's status is in the browser's store. */
const storedStatus = (page: Page, slug: string, alert: string) =>
	page.evaluate(
		([reader, slug, alert]) =>
			new Promise<string | undefined>((resolve) => {
				const open = indexedDB.open("bencher-console");
				open.onerror = () => resolve(undefined);
				open.onsuccess = () => {
					const read = open.result
						.transaction("cache")
						.objectStore("cache")
						.get(reader);
					read.onerror = () => resolve(undefined);
					read.onsuccess = () => {
						const queries: {
							queryKey: unknown[];
							state: { data?: { status?: string } };
						}[] = read.result?.state?.queries ?? [];
						resolve(
							queries.find(
								({ queryKey }) =>
									JSON.stringify(queryKey) ===
									JSON.stringify(["console", "alert", slug, alert]),
							)?.state.data?.status,
						);
					};
				};
			}),
		[seed.member.user.uuid, slug, alert] as const,
	);

const patched = (page: Page) =>
	page.waitForResponse(
		(response) =>
			/\/v0\/console\/projects\/[^/]+\/alerts$/.test(response.url()) &&
			response.request().method() === "PATCH",
	);

/** Records in the tab's session each document that painted before it went. */
const recordPaints = (page: Page) =>
	page.addInitScript(() => {
		addEventListener("pagehide", () => {
			if (performance.getEntriesByType("paint").length > 0) {
				const painted = JSON.parse(sessionStorage.getItem("painted") ?? "[]");
				painted.push(location.pathname);
				sessionStorage.setItem("painted", JSON.stringify(painted));
			}
		});
	});
/** The console's documents that painted before they went. */
const consolePaints = async (page: Page) =>
	(
		(await page.evaluate(() =>
			JSON.parse(sessionStorage.getItem("painted") ?? "[]"),
		)) as string[]
	).filter((path) => path.startsWith("/console/") || path.startsWith("/perf/"));

/** Explore drawing the alert's line, as its page's Open in Explore opens it. */
const exploreOf = async (page: Page, slug: string, alert: string) => {
	await page.goto(alertPage(slug, alert));
	const href = await page
		.getByRole("link", { name: /^Open in Explore: / })
		.getAttribute("href");
	if (!href) {
		throw new Error("The alert's page has no Explore link");
	}
	return href;
};

/** Pins the readout of the only plot on the page at its last point, the alert's. */
const pinLast = async (page: Page) => {
	await page.getByRole("group", { name: /^1 line/ }).focus();
	await page.keyboard.press("End");
};

test.describe("signed in", () => {
	test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

	// Kills an Alerts row whose name opens nothing, or opens another alert's
	// page, a page under another tab, a plot that opens with nothing pinned, and
	// a tab title that names the alert's tab rather than its benchmark, or keeps
	// it after the page leaves.
	test("an Alerts row's name opens its alert's page, pinned on the alert", async ({
		page,
		request,
	}) => {
		const alert = await alertOf(request, hashbrown.slug, "blake3");
		await page.goto(consolePath(hashbrown.slug, "alerts"));
		await page
			.getByRole("link", { name: /^blake3 .*, open the alert$/ })
			.click();
		await expect(page).toHaveURL(alertPage(hashbrown.slug, alert.uuid));
		await expect(heading(page)).toContainText("blake3");
		await expect(dismiss(page, BLAKE3)).toBeVisible();
		await expect(
			tabRow(page).getByRole("link", { name: /^Alerts/ }),
		).toHaveAttribute("aria-current", "page");
		await expect(readout(page)).toContainText("20.60 ns");
		await expect(readout(page)).toHaveAttribute("aria-live", "polite");
		await expect(page).toHaveTitle(
			`blake3 | Alert | ${hashbrown.name} | Bencher`,
		);
		await page.goBack();
		await expect(page).toHaveTitle(`Alerts | ${hashbrown.name} | Bencher`);
	});

	// Kills an alert page drawn as a failure, or as some alert, for a UUID no alert has.
	test("an unknown alert is not found", async ({ page }) => {
		await page.goto(alertPage(hashbrown.slug, randomUUID()));
		await expect(heading(page)).toHaveText("Alert not found");
	});

	// Kills a page any of whose colors, names, or targets fail axe in either theme, wide or on a phone.
	test("an alert's page passes axe in both themes", async ({
		page,
		request,
	}) => {
		const alert = await alertOf(request, hashbrown.slug, "blake3");
		await axeInBothThemes(
			page,
			alertPage(hashbrown.slug, alert.uuid),
			async () => {
				await expect(dismiss(page, BLAKE3)).toBeVisible();
				await expect(readout(page)).toHaveAttribute("aria-live", "polite");
				await expect(
					page.getByRole("link", { name: `Open in Explore: ${BLAKE3}` }),
				).toBeVisible();
			},
		);
	});

	// Kills a pinned readout on an alert whose colors, names, or targets fail
	// axe in either theme, wide or on a phone, or whose Alert link is named
	// apart from the word it shows.
	test("a pinned readout on an alert passes axe in both themes", async ({
		page,
		request,
	}) => {
		const alert = await alertOf(request, hashbrown.slug, "blake3");
		await axeInBothThemes(
			page,
			await exploreOf(page, hashbrown.slug, alert.uuid),
			async () => {
				await expect(alertLink(page, BLAKE3)).toBeVisible();
				await expect(dismiss(page, BLAKE3)).toBeVisible();
			},
			() => pinLast(page),
		);
	});

	// Kills a refused Dismiss from a pinned readout that says nothing where it was made.
	test("a refused Dismiss from a pinned readout in Explore says so there", async ({
		page,
		request,
	}) => {
		const alert = await alertOf(request, hashbrown.slug, "blake3");
		await page.goto(await exploreOf(page, hashbrown.slug, alert.uuid));
		await page.route(
			`${seed.api_url}/v0/console/projects/${hashbrown.slug}/alerts`,
			(route) =>
				route.request().method() === "PATCH"
					? route.fulfill({ status: 500, json: {} })
					: route.fallback(),
		);
		await pinLast(page);
		await dismiss(page, BLAKE3).click();
		await expect(readout(page).getByRole("alert")).toBeVisible();
		await expect(dismiss(page, BLAKE3)).toBeVisible();
	});

	test.describe("on a project of its own", () => {
		// Kills a Dismiss that leaves the Alerts count or the list as they were,
		// and a Reactivate that does not undo it.
		test("Dismiss and Reactivate on the page move the Alerts count, and the list follows", async ({
			page,
			request,
		}) => {
			const { project } = await createAlerts(request);
			const alert = await alertOf(request, project.slug, "bench-02");
			const name = line("bench-02");
			await page.goto(alertPage(project.slug, alert.uuid));
			await expect(badge(page, 3)).toBeVisible();

			const taken = patched(page);
			await dismiss(page, name).click();
			await expect(reactivate(page, name)).toBeFocused();
			await expect(badge(page, 2)).toBeVisible();
			await taken;
			await tabRow(page)
				.getByRole("link", { name: /^Alerts/ })
				.click();
			await expect(heading(page)).toHaveText("2 active");
			expect(await alertRows(page)).not.toContain(name);

			await page.goBack();
			const undone = patched(page);
			await reactivate(page, name).click();
			await expect(dismiss(page, name)).toBeFocused();
			await expect(badge(page, 3)).toBeVisible();
			await undone;
			await tabRow(page)
				.getByRole("link", { name: /^Alerts/ })
				.click();
			await expect(heading(page)).toHaveText("3 active");
			expect(await alertRows(page)).toContain(name);
		});

		// Kills a Dismiss the browser's store forgets, so a reload shows the
		// alert active until the API answers.
		test("a dismissed alert's page shows it dismissed after a reload, before the API answers", async ({
			page,
			request,
		}) => {
			const { project } = await createAlerts(request);
			const alert = await alertOf(request, project.slug, "bench-02");
			const name = line("bench-02");
			await page.goto(alertPage(project.slug, alert.uuid));
			const taken = patched(page);
			await dismiss(page, name).click();
			await taken;
			await expect
				.poll(() => storedStatus(page, project.slug, alert.uuid))
				.toBe("dismissed");

			const held: (() => void)[] = [];
			const path = `${seed.api_url}/v0/console/projects/${project.slug}/alerts/${alert.uuid}`;
			await page.route(path, async (route) => {
				await new Promise<void>((resolve) => held.push(resolve));
				await route.fallback();
			});
			await page.reload();
			await expect(reactivate(page, name)).toBeVisible();
			await expect.poll(() => held.length).toBe(1);
			await page.unroute(path);
			for (const release of held) {
				release();
			}
		});

		// Kills a readout on an alert with no Dismiss, a Dismiss that leaves the
		// Alerts count or the list as they were, and one that does not offer
		// Reactivate in its place.
		test("Dismiss from a pinned readout in Explore moves the Alerts count, and the list follows", async ({
			page,
			request,
		}) => {
			const { project } = await createAlerts(request);
			const alert = await alertOf(request, project.slug, "bench-02");
			const name = line("bench-02");
			await page.goto(alertPage(project.slug, alert.uuid));
			await page
				.getByRole("link", { name: `Open in Explore: ${name}` })
				.click();
			await expect(page).toHaveURL(/\/explore\?/);
			await pinLast(page);
			await expect(alertLink(page, name)).toHaveAttribute(
				"href",
				alertPage(project.slug, alert.uuid),
			);
			await expect(badge(page, 3)).toBeVisible();

			const taken = patched(page);
			await dismiss(page, name).click();
			await expect(reactivate(page, name)).toBeFocused();
			await expect(readout(page)).toHaveAttribute("aria-live", "polite");
			await expect(badge(page, 2)).toBeVisible();
			await taken;
			await tabRow(page)
				.getByRole("link", { name: /^Alerts/ })
				.click();
			await expect(heading(page)).toHaveText("2 active");
			expect(await alertRows(page)).not.toContain(name);
		});

		// Kills a readout's Reactivate that sends nothing, leaves the Alerts
		// count or the list without the alert, or unpins the readout.
		test("Reactivate from a pinned readout in Explore restores the Alerts count, and the list follows", async ({
			page,
			request,
		}) => {
			const { project } = await createAlerts(request);
			const alert = await alertOf(request, project.slug, "bench-02");
			const name = line("bench-02");
			await page.goto(alertPage(project.slug, alert.uuid));
			await page
				.getByRole("link", { name: `Open in Explore: ${name}` })
				.click();
			await expect(page).toHaveURL(/\/explore\?/);
			await pinLast(page);
			const dismissed = patched(page);
			await dismiss(page, name).click();
			await expect(badge(page, 2)).toBeVisible();
			await dismissed;

			const reactivated = patched(page);
			await reactivate(page, name).click();
			await expect(dismiss(page, name)).toBeFocused();
			await expect(readout(page)).toHaveAttribute("aria-live", "polite");
			await expect(badge(page, 3)).toBeVisible();
			expect((await reactivated).request().postDataJSON()).toEqual({
				status: "active",
				alerts: [alert.uuid],
			});
			await tabRow(page)
				.getByRole("link", { name: /^Alerts/ })
				.click();
			await expect(heading(page)).toHaveText("3 active");
			expect(await alertRows(page)).toContain(name);
		});
	});

	// Kills a page that draws an alert's old status when the reader comes back
	// to it, until it reads again.
	test("a return to Explore after a Dismiss from its readout draws the alert dismissed at once", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		const alert = await alertOf(request, project.slug, "bench-02");
		const name = line("bench-02");
		await page.goto(await exploreOf(page, project.slug, alert.uuid));
		await pinLast(page);
		const taken = patched(page);
		await dismiss(page, name).click();
		await taken;
		await tabRow(page)
			.getByRole("link", { name: /^Alerts/ })
			.click();
		await expect(heading(page)).toHaveText("2 active");

		const held: (() => void)[] = [];
		const perf = `${seed.api_url}/v0/console/projects/${project.slug}/perf**`;
		await page.route(perf, async (route) => {
			await new Promise<void>((resolve) => held.push(resolve));
			await route.fallback();
		});
		await page.goBack();
		await pinLast(page);
		await expect(reactivate(page, name)).toBeVisible();
		await expect(page.locator("figure .pl-pill.alert")).toHaveCount(0);
		await expect.poll(() => held.length).toBe(1);
		await page.unroute(perf);
		for (const release of held) {
			release();
		}
	});

	// Kills a return that draws the first of two changes made on the page, so a
	// Dismiss undone by Reactivate comes back dismissed until the reread.
	test("a return to Explore after a Dismiss and a Reactivate from its readout draws the alert active at once", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		const alert = await alertOf(request, project.slug, "bench-02");
		const name = line("bench-02");
		await page.goto(await exploreOf(page, project.slug, alert.uuid));
		await pinLast(page);
		const dismissed = patched(page);
		await dismiss(page, name).click();
		await dismissed;
		const reactivated = patched(page);
		await reactivate(page, name).click();
		await reactivated;
		await tabRow(page)
			.getByRole("link", { name: /^Alerts/ })
			.click();
		await expect(heading(page)).toHaveText("3 active");

		const held: (() => void)[] = [];
		const perf = `${seed.api_url}/v0/console/projects/${project.slug}/perf**`;
		await page.route(perf, async (route) => {
			await new Promise<void>((resolve) => held.push(resolve));
			await route.fallback();
		});
		await page.goBack();
		await pinLast(page);
		await expect(dismiss(page, name)).toBeVisible();
		await expect(page.locator("figure .pl-pill.alert")).toHaveCount(1);
		await expect.poll(() => held.length).toBe(1);
		await page.unroute(perf);
		for (const release of held) {
			release();
		}
	});

	// Kills a classic alert link that keeps a version 1 project in the classic
	// console, or sends it anywhere but the alert's page.
	test("the classic console and perf alert links open the alert's page", async ({
		page,
		request,
	}) => {
		const alert = await alertOf(request, hashbrown.slug, "blake3");
		for (const classic of [
			`/console/projects/${hashbrown.slug}/alerts/${alert.uuid}`,
			`/perf/${hashbrown.slug}/alerts/${alert.uuid}`,
		]) {
			await page.goto(classic);
			await expect(page).toHaveURL(alertPage(hashbrown.slug, alert.uuid));
			await expect(heading(page)).toContainText("blake3");
		}
	});

	// Kills a version 0 project's classic alert links kept out of the console.
	test("a version 0 project's classic alert links open the alert's page", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		const alert = await alertOf(request, project.slug, "bench-02");
		const moved = await request.fetch(
			`${seed.api_url}/v0/projects/${project.slug}`,
			{
				method: "PATCH",
				headers: { Authorization: `Bearer ${seed.admin.token}` },
				data: { bmf_version: 0 },
			},
		);
		expect(moved.ok()).toBe(true);
		for (const classic of [
			`/console/projects/${project.slug}/alerts/${alert.uuid}`,
			`/perf/${project.slug}/alerts/${alert.uuid}`,
		]) {
			await page.goto(classic);
			await expect(page).toHaveURL(alertPage(project.slug, alert.uuid));
			await expect(heading(page)).toContainText("bench-02");
		}
	});
});

test.describe("signed out", () => {
	test.use({ storageState: signedOut, timezoneId: "UTC" });
	test.beforeEach(({ page }) => recordPaints(page));

	// Kills a signed-out reader of a classic alert link sent anywhere but sign
	// in, one sent there without the alert page to return to, and a console
	// page that paints before it moves.
	test("a classic alert link sends a signed-out reader to sign in and back to the alert page", async ({
		page,
		request,
	}) => {
		const alert = await alertOf(request, hashbrown.slug, "blake3");
		for (const classic of [
			`/perf/${hashbrown.slug}/alerts/${alert.uuid}`,
			`/console/projects/${hashbrown.slug}/alerts/${alert.uuid}`,
		]) {
			await page.goto(classic);
			await expect(page).toHaveURL(
				logInTo(alertPage(hashbrown.slug, alert.uuid)),
			);
		}
		const closed = seed.projects.private;
		const unknown = alertPage(closed.slug, randomUUID());
		await page.goto(unknown);
		await expect(page).toHaveURL(logInTo(unknown));
		expect(await consolePaints(page)).toEqual([]);
	});
});

test.describe("a reader who is not a member", () => {
	test.use({ storageState: signedIn(seed.outsider), timezoneId: "UTC" });

	// Kills a viewer offered a Dismiss in a pinned readout in Explore, which the API refuses.
	test("a pinned readout in Explore links the alert and offers no change", async ({
		page,
		request,
	}) => {
		const alert = await alertOf(request, hashbrown.slug, "blake3");
		await page.goto(await exploreOf(page, hashbrown.slug, alert.uuid));
		await pinLast(page);
		await expect(alertLink(page, BLAKE3)).toHaveAttribute(
			"href",
			alertPage(hashbrown.slug, alert.uuid),
		);
		await expect(
			readout(page).getByRole("button", { name: /the alert on/ }),
		).toHaveCount(0);
	});
});
