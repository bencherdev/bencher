import type { APIRequestContext, Locator, Page } from "@playwright/test";
import { createAlerts, createManyAlerts, raiseAlerts } from "./api";
import { axeInBothThemes } from "./axe";
import {
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;

const BLAKE3 = "blake3 input_bytes=65536 simd=avx2 threads=1 Latency";
const SHA256 = "sha256 input_bytes=65536 simd=avx2 threads=4 Latency";
const line = (benchmark: string) => `${benchmark} n=0 Latency`;
/** The seed's newest `main` report, which raised the blake3 alert. */
const MAIN_REPORT = "main, ubuntu-latest, Sep 13, 21:16";
// The reports `createAlerts` posts, by their branch, testbed, and time.
const OLDER = "main, ubuntu-latest, Sep 13, 20:00";
const NEWER = "main, ubuntu-latest, Sep 13, 21:00";
const SILENCED = "feature, ubuntu-latest, Sep 13, 23:00";

const alerts = (slug: string, search = "") =>
	`${nextPath(slug, "alerts")}${search}`;
const heading = (page: Page) => page.getByRole("heading", { level: 1 });
const status = (page: Page, name: string) =>
	page.getByRole("radiogroup", { name: "Status" }).getByRole("radio", { name });
const table = (page: Page) => page.getByRole("table", { name: /^Alerts/ });
const row = (page: Page, name: string) =>
	table(page)
		.getByRole("row")
		.filter({ has: page.getByRole("checkbox", { name: `Select ${name}` }) });
const groupRow = (page: Page, report: string) =>
	table(page)
		.getByRole("row")
		.filter({
			has: page.getByRole("checkbox", {
				name: `Select the alerts in the report on ${report}`,
			}),
		});
/** Each drawn row's text, group headers included, in order. */
const rowNames = (page: Page) =>
	table(page)
		.getByRole("checkbox")
		.evaluateAll((boxes) =>
			boxes.map((box) => (box.getAttribute("aria-label") ?? "").slice(7)),
		);
const badge = (page: Page, active: number) =>
	tabRow(page).getByRole("link", {
		name: active === 0 ? "Alerts" : `Alerts ${active} active`,
		exact: true,
	});
const dismiss = (scope: Page | Locator, name: string) =>
	scope.getByRole("button", { name: `Dismiss the alert on ${name}` });
const reactivate = (scope: Page | Locator, name: string) =>
	scope.getByRole("button", { name: `Reactivate the alert on ${name}` });

/** The alert's report on the seed, by the reports list. */
const newestMain = async (request: APIRequestContext) => {
	const response = await request.get(
		`${seed.api_url}/v0/projects/${hashbrown.slug}/reports?branch=main&testbed=ubuntu-latest&per_page=1`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	const [report] = (await response.json()) as { uuid: string }[];
	if (!report) {
		throw new Error("The seed has no main report");
	}
	return report.uuid;
};

/** The project's alerts by status, as the API holds them. */
const apiCounts = async (request: APIRequestContext, slug: string) => {
	const response = await request.get(
		`${seed.api_url}/v0/projects/${slug}/console/alerts?status=all&per_page=0`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	return ((await response.json()) as { counts: Record<string, number> }).counts;
};
/** Every read of the project's alert list, held until `release`, counted. */
const holdLists = async (page: Page, slug: string) => {
	const held: (() => void)[] = [];
	await page.route(
		`${seed.api_url}/v0/projects/${slug}/console/alerts?*`,
		async (route) => {
			await new Promise<void>((resolve) => held.push(resolve));
			await route.fallback();
		},
	);
	return {
		asked: () => held.length,
		release: async () => {
			await page.unroute(
				`${seed.api_url}/v0/projects/${slug}/console/alerts?*`,
			);
			for (const resolve of held) {
				resolve();
			}
		},
	};
};

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

// Kills an alert drawn outside its report's group, a group missing its
// branch, testbed, time, hash, or count, an Open report that leads elsewhere,
// a count that is not the active total, and an alert that is not the report
// page's own row.
test("the active blake3 alert sits under report 9c1f2e4 with its value, delta, and limit", async ({
	page,
	request,
}) => {
	await page.goto(alerts(hashbrown.slug));

	await expect(heading(page)).toHaveText("1 active");
	await expect(
		page.getByText("1 dismissed and 0 silenced in the last 4 weeks"),
	).toBeVisible();
	expect(await rowNames(page)).toEqual([
		`the alerts in the report on ${MAIN_REPORT}`,
		BLAKE3,
	]);
	const group = groupRow(page, MAIN_REPORT);
	await expect(group).toContainText("main · ubuntu-latest");
	await expect(group).toContainText("Sep 13, 21:16 · 9c1f2e4 · json · 1 alert");
	await expect(
		group.getByRole("link", { name: `Open report on ${MAIN_REPORT}` }),
	).toHaveAttribute(
		"href",
		`${nextPath(hashbrown.slug, "reports")}/${await newestMain(request)}`,
	);
	const blake3 = row(page, BLAKE3);
	await expect(blake3).toContainText("20.60 ns");
	await expect(blake3).toContainText("+6.2% worse");
	await expect(
		blake3.getByRole("img", { name: `${BLAKE3}, history` }),
	).toBeVisible();
	await expect(blake3.getByRole("img", { name: "alerting" })).toBeVisible();
	await expect(dismiss(blake3, BLAKE3)).toBeVisible();

	await blake3.getByRole("button", { name: `Expand ${BLAKE3}` }).click();
	await expect(
		page
			.getByRole("region", { name: `${BLAKE3}, full plot` })
			.getByRole("button", { name: /^Hide / }),
	).toBeVisible();
});

// Kills a status toggle outside the URL, a Dismissed view that hides its
// alerts' status or drops Reactivate, and a dismissed row told apart by
// opacity rather than by color.
test("Dismissed lists the dismissed alert dimmed with Reactivate, and the status rides in the link", async ({
	page,
}) => {
	await page.goto(alerts(hashbrown.slug));
	await status(page, "Dismissed").check();
	await expect(page).toHaveURL(/[?&]status=dismissed/);

	const sha256 = row(page, SHA256);
	await expect(sha256).toContainText(/Dismissed [A-Z][a-z]{2} \d{1,2}/);
	await expect(reactivate(sha256, SHA256)).toBeVisible();
	await expect(dismiss(sha256, SHA256)).toHaveCount(0);
	await expect(sha256.getByRole("img", { name: "alerting" })).toHaveCount(0);
	const style = await sha256.evaluate((element) => {
		const name = element.querySelector("b");
		const probe = document.createElement("span");
		probe.style.color = "var(--color-text-muted)";
		document.body.append(probe);
		const muted = getComputedStyle(probe).color;
		probe.remove();
		return {
			opacity: [element, ...element.querySelectorAll("td")].map(
				(each) => getComputedStyle(each).opacity,
			),
			name: name ? getComputedStyle(name).color : "",
			muted,
		};
	});
	expect(new Set(style.opacity)).toEqual(new Set(["1"]));
	expect(style.name).toBe(style.muted);

	await page.reload();
	await expect(status(page, "Dismissed")).toBeChecked();
	await expect(row(page, SHA256)).toBeVisible();
	await expect(row(page, BLAKE3)).toHaveCount(0);
});

test.describe("on a project of its own", () => {
	// Kills a dismiss that waits for the API to grey the row, one that removes
	// the row instead, a badge that keeps its count, and a Reactivate that
	// leaves the row grey or the badge short.
	test("dismissing greys the row in place with Reactivate and moves the badge, and Reactivate restores both", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		await page.goto(alerts(project.slug));
		await expect(heading(page)).toHaveText("3 active");
		await expect(badge(page, 3)).toBeVisible();

		const bench02 = row(page, line("bench-02"));
		await dismiss(bench02, line("bench-02")).click();
		await expect(bench02).toContainText("Dismissed just now");
		await expect(reactivate(bench02, line("bench-02"))).toBeVisible();
		await expect(heading(page)).toHaveText("2 active");
		await expect(badge(page, 2)).toBeVisible();
		await settle(page);
		// Still in place once the list is read again.
		await status(page, "All").check();
		await status(page, "Active").check();
		await expect(row(page, line("bench-02"))).toContainText(
			"Dismissed just now",
		);

		await reactivate(row(page, line("bench-02")), line("bench-02")).click();
		await expect(
			dismiss(row(page, line("bench-02")), line("bench-02")),
		).toBeVisible();
		await expect(heading(page)).toHaveText("3 active");
		await expect(badge(page, 3)).toBeVisible();
		await settle(page);
		await page.reload();
		await expect(heading(page)).toHaveText("3 active");
	});

	// Kills a refusal that leaves the row grey, the count or the badge moved,
	// or says nothing.
	test("a refused dismiss rolls back with an error", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		await page.route(
			`${seed.api_url}/v0/projects/${project.slug}/console/alerts`,
			(route) =>
				route.request().method() === "PATCH"
					? route.fulfill({ status: 500, body: "{}" })
					: route.fallback(),
		);
		await page.goto(alerts(project.slug));
		await expect(heading(page)).toHaveText("3 active");

		await dismiss(row(page, line("bench-02")), line("bench-02")).click();
		await expect(page.getByRole("alert")).toContainText(
			"did not change: the Bencher API did not answer",
		);
		await expect(
			dismiss(row(page, line("bench-02")), line("bench-02")),
		).toBeVisible();
		await expect(heading(page)).toHaveText("3 active");
		await expect(badge(page, 3)).toBeVisible();
	});

	// Kills a Dismiss all that skips the confirmation, counts what is loaded
	// rather than what matches, or leaves an active alert behind.
	test("Dismiss all confirms with the count and clears the page", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		await page.goto(alerts(project.slug));
		await page
			.getByRole("button", {
				name: "Dismiss all 3 active alerts that match the filters",
			})
			.click();
		const dialog = page.getByRole("dialog", {
			name: "Dismiss 3 active alerts?",
		});
		await expect(dialog).toContainText("the last 4 weeks");
		await dialog.getByRole("button", { name: "Dismiss 3 alerts" }).click();
		await expect(dialog).toBeHidden();
		await expect(heading(page)).toHaveText("0 active");
		await expect(badge(page, 0)).toBeVisible();
		await expect(table(page).getByText("Dismissed just now")).toHaveCount(3);
		await settle(page);

		await page.reload();
		await expect(
			page.getByRole("heading", { name: "No active alerts" }),
		).toBeVisible();
	});

	// Kills an All view that hides a status, a silenced alert offered
	// Reactivate or shown without its reason, and Reactivate N counting a
	// silenced alert.
	test("All shows every status: dismissed with Reactivate, silenced with its reason and none", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		await page.goto(alerts(project.slug, "?status=all"));
		await expect(status(page, "All")).toBeChecked();
		expect(await rowNames(page)).toEqual([
			`the alerts in the report on ${SILENCED}`,
			line("bench-00"),
			`the alerts in the report on ${NEWER}`,
			line("bench-02"),
			`the alerts in the report on ${OLDER}`,
			line("bench-00"),
			line("bench-01"),
		]);
		const silenced = table(page)
			.getByRole("row")
			.filter({ hasText: "Silenced: the branch head was replaced" });
		await expect(silenced).toHaveCount(1);
		await expect(
			silenced.getByRole("button", { name: `Expand ${line("bench-00")}` }),
		).toBeVisible();
		await expect(
			silenced.getByRole("button", { name: /^(Reactivate|Dismiss)/ }),
		).toHaveCount(0);

		await groupRow(page, SILENCED).getByRole("checkbox").check();
		await groupRow(page, NEWER).getByRole("checkbox").check();
		await expect(page.getByText("2 selected")).toBeVisible();
		await expect(
			page.getByRole("button", { name: /^Reactivate \d/ }),
		).toHaveCount(0);
		await page
			.getByRole("button", { name: "Dismiss 1 selected active alert" })
			.click();
		await expect(row(page, line("bench-02")).first()).toContainText(
			"Dismissed just now",
		);
		await expect(
			page.getByRole("button", {
				name: "Reactivate 1 selected dismissed alert",
			}),
		).toBeVisible();
	});

	// Kills a group checkbox that does not show a partial selection, selects
	// beyond its group, or clears only part of it; and Dismiss group acting on
	// rows outside its report.
	test("a group's checkbox is tri-state, and Dismiss group dismisses only that report", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		await page.goto(alerts(project.slug));
		const older = groupRow(page, OLDER).getByRole("checkbox");

		await row(page, line("bench-00")).getByRole("checkbox").check();
		await expect(older).toHaveJSProperty("indeterminate", true);
		await expect(older).not.toBeChecked();
		await expect(page.getByText("1 selected")).toBeVisible();

		await older.check();
		await expect(older).toHaveJSProperty("indeterminate", false);
		await expect(
			row(page, line("bench-01")).getByRole("checkbox"),
		).toBeChecked();
		await expect(page.getByText("2 selected")).toBeVisible();
		await expect(
			row(page, line("bench-02")).getByRole("checkbox"),
		).not.toBeChecked();

		await older.uncheck();
		await expect(
			page.getByText("Select alerts to act on them together"),
		).toBeVisible();

		await groupRow(page, OLDER)
			.getByRole("button", { name: `Dismiss group on ${OLDER}` })
			.click();
		await expect(row(page, line("bench-00"))).toContainText(
			"Dismissed just now",
		);
		await expect(row(page, line("bench-01"))).toContainText(
			"Dismissed just now",
		);
		await expect(
			dismiss(row(page, line("bench-02")), line("bench-02")),
		).toBeVisible();
		await expect(heading(page)).toHaveText("1 active");
		await settle(page);
		await page.reload();
		expect(await rowNames(page)).toEqual([
			`the alerts in the report on ${NEWER}`,
			line("bench-02"),
		]);
	});

	// Kills a filter left out of the link or the request, and a status change
	// that leaves the Reports list's cached counts behind.
	test("a branch filter rides in the link, and a dismiss reaches the Reports list at once", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		await page.goto(alerts(project.slug, "?status=all"));
		await page.getByRole("button", { name: "Filter by branch, any" }).click();
		await page.getByRole("menuitemradio", { name: "feature" }).click();
		await expect(page).toHaveURL(/[?&]branch=[0-9a-f-]{36}/);
		await expect(heading(page)).toHaveText("0 active");
		expect(await rowNames(page)).toEqual([
			`the alerts in the report on ${SILENCED}`,
			line("bench-00"),
		]);
		await page.reload();
		await expect(
			page.getByRole("button", { name: "Filter by branch, feature" }),
		).toBeVisible();
		await expect(row(page, line("bench-00"))).toContainText(
			"Silenced: the branch head was replaced",
		);

		// The Reports list is read and cached, then the page dismisses an alert it counts.
		await tabRow(page).getByRole("link", { name: "Reports" }).click();
		await expect(page.getByRole("heading", { level: 1 })).toHaveText(
			"6 reports",
		);
		// Newest first: the newer `main` report is the fourth.
		const newerCount = page
			.getByRole("table", { name: "Reports, newest first" })
			.getByRole("rowgroup")
			.last()
			.getByRole("row")
			.nth(3)
			.getByRole("cell")
			.nth(6);
		await expect(newerCount).toHaveText(/^1 active\s*1 total$/);
		await tabRow(page)
			.getByRole("link", { name: /^Alerts/ })
			.click();
		await dismiss(row(page, line("bench-02")), line("bench-02")).click();
		await expect(heading(page)).toHaveText("2 active");
		await settle(page);
		await tabRow(page).getByRole("link", { name: "Reports" }).click();
		await expect(newerCount).toHaveText(/^1 total$/);
	});
});

test.describe("returning to the list", () => {
	// Kills a dismiss that the list it was made in forgets once the reader
	// leaves, so a return shows the alert active again, and a list the return
	// never reads again from the API.
	test("a dismissed alert stays dismissed when the reader leaves and comes back", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		await page.goto(alerts(project.slug));
		await expect(heading(page)).toHaveText("3 active");
		await dismiss(row(page, line("bench-02")), line("bench-02")).click();
		await expect(heading(page)).toHaveText("2 active");
		await settle(page);

		await tabRow(page).getByRole("link", { name: "Reports" }).click();
		await expect(page.getByRole("heading", { level: 1 })).toHaveText(
			"6 reports",
		);
		const lists = await holdLists(page, project.slug);
		await tabRow(page)
			.getByRole("link", { name: /^Alerts/ })
			.click();
		await expect(heading(page)).toHaveText("2 active");
		expect(await rowNames(page)).not.toContain(line("bench-02"));
		await expect.poll(lists.asked).toBe(1);
		await lists.release();
		await settle(page);
		await expect(heading(page)).toHaveText("2 active");
		expect(await rowNames(page)).not.toContain(line("bench-02"));
	});

	// Kills a dismiss the browser's store forgets, so a reload shows the alert
	// active again until the API answers.
	test("a dismissed alert stays dismissed after a reload", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		await page.goto(alerts(project.slug));
		await expect(heading(page)).toHaveText("3 active");
		await dismiss(row(page, line("bench-02")), line("bench-02")).click();
		await expect(heading(page)).toHaveText("2 active");
		// The cache persists to IndexedDB a moment after it changes.
		await settle(page, 1_500);

		const lists = await holdLists(page, project.slug);
		await page.reload();
		await expect(heading(page)).toHaveText("2 active");
		expect(await rowNames(page)).not.toContain(line("bench-02"));
		await expect.poll(lists.asked).toBe(1);
		await lists.release();
		await settle(page);
		expect(await rowNames(page)).not.toContain(line("bench-02"));
	});
});

test.describe("Dismiss all", () => {
	// Kills a Dismiss all whose window has no end, which dismisses an alert
	// raised after the list was read that the reader never saw or counted,
	// and a count that then goes below zero.
	test("leaves active an alert raised after the list was read", async ({
		page,
		request,
	}) => {
		const { project } = await createAlerts(request);
		const listed = page.waitForResponse(
			(response) =>
				response.url().includes("/console/alerts?") &&
				response.request().method() === "GET",
		);
		await page.goto(alerts(project.slug));
		const { read_time } = (await (await listed).json()) as {
			read_time: number;
		};
		await expect(heading(page)).toHaveText("3 active");
		await settle(page);
		// A report's creation is kept to the second, so the new one comes in the second after the read.
		await expect
			.poll(() => Date.now(), { intervals: [100] })
			.toBeGreaterThanOrEqual(Math.floor(read_time / 1_000) * 1_000 + 1_000);
		await raiseAlerts(request, project.slug, 90, ["bench-01"]);

		await page
			.getByRole("button", {
				name: "Dismiss all 3 active alerts that match the filters",
			})
			.click();
		await page
			.getByRole("dialog", { name: "Dismiss 3 active alerts?" })
			.getByRole("button", { name: "Dismiss 3 alerts" })
			.click();
		await expect(heading(page)).toHaveText("0 active");
		await settle(page);
		expect(await apiCounts(request, project.slug)).toEqual({
			active: 1,
			dismissed: 3,
			silenced: 1,
		});
		await expect(heading(page)).toHaveText("0 active");
		await expect(badge(page, 0)).toBeVisible();
	});

	test.describe("on a phone held sideways", () => {
		test.use({ viewport: { width: 844, height: 390 } });

		// Kills a dialog that runs past the bottom of a short screen, whose
		// confirm only a keyboard reaches.
		test("its confirm is in reach of a pointer", async ({ page, request }) => {
			const { project } = await createAlerts(request);
			await page.goto(alerts(project.slug));
			await page
				.getByRole("button", {
					name: "Dismiss all 3 active alerts that match the filters",
				})
				.click();
			const confirm = page
				.getByRole("dialog", { name: "Dismiss 3 active alerts?" })
				.getByRole("button", { name: "Dismiss 3 alerts" });
			const box = await confirm.boundingBox();
			expect((box?.y ?? 0) + (box?.height ?? 0)).toBeLessThanOrEqual(390);
			await page.mouse.click(
				(box?.x ?? 0) + (box?.width ?? 0) / 2,
				(box?.y ?? 0) + (box?.height ?? 0) / 2,
			);
			await expect(heading(page)).toHaveText("0 active");
		});
	});
});

// Kills a page that scrolls sideways between tablet and desktop widths, and
// row or group controls drawn past the edge of the screen or of the list.
for (const width of [768, 1024]) {
	test(`at ${width} px nothing scrolls sideways and every control is in view`, async ({
		page,
		request,
	}) => {
		await page.setViewportSize({ width, height: 900 });
		const { project } = await createAlerts(request);
		await page.goto(alerts(project.slug, "?status=all"));
		await groupRow(page, OLDER)
			.getByRole("button", { name: `Dismiss group on ${OLDER}` })
			.click();
		await expect(
			reactivate(row(page, line("bench-01")), line("bench-01")),
		).toBeVisible();
		await settle(page);
		const measured = await page.evaluate(() => {
			const list = document
				.querySelector(".al-scroll")
				?.getBoundingClientRect();
			const right = Math.min(window.innerWidth, list?.right ?? 0);
			return {
				page: document.documentElement.scrollWidth,
				controls: document.querySelectorAll(".al-scroll button, .al-scroll a")
					.length,
				out: [...document.querySelectorAll(".al-scroll button, .al-scroll a")]
					.filter((control) => {
						const box = control.getBoundingClientRect();
						return (
							box.right > right + 0.5 || box.left < (list?.left ?? 0) - 0.5
						);
					})
					.map((control) => control.getAttribute("aria-label")),
			};
		});
		expect(measured.page).toBeLessThanOrEqual(width);
		expect(measured.controls).toBeGreaterThan(0);
		expect(measured.out).toEqual([]);
	});
}

test.describe("a list longer than a batch", () => {
	test.use({ viewport: { width: 1280, height: 400 } });

	// Kills a next batch that skips the alerts dismissed rows made room for,
	// or repeats one, and a batch asked for by page number.
	test("dismissing rows on screen, then scrolling, skips and repeats no alert", async ({
		page,
		request,
	}) => {
		const project = await createManyAlerts(request, 60);
		const batches: URLSearchParams[] = [];
		page.on("request", (sent) => {
			const url = new URL(sent.url());
			if (url.pathname.endsWith("/console/alerts") && sent.method() === "GET") {
				batches.push(url.searchParams);
			}
		});
		await page.goto(alerts(project.slug));
		await expect(heading(page)).toHaveText("60 active");
		await settle(page);
		const size = Number(batches[0]?.get("per_page"));
		expect(batches.length * size).toBeLessThan(60);
		const dismissable = table(page).getByRole("button", {
			name: /^Dismiss the alert on /,
		});
		for (const count of [1, 2, 3]) {
			await dismissable.first().click();
			await expect(table(page).getByText("Dismissed just now")).toHaveCount(
				count,
			);
		}
		await expect(heading(page)).toHaveText("57 active");

		// Every alert once: the header, the report's row, and sixty alerts.
		await expect(async () => {
			await page.mouse.wheel(0, 5_000);
			await expect(table(page)).toHaveAttribute("aria-rowcount", "62", {
				timeout: 1_000,
			});
		}).toPass();
		await settle(page);
		await expect(table(page)).toHaveAttribute("aria-rowcount", "62");
		const offsets = batches.map((batch) => Number(batch.get("offset")));
		expect(batches.every((batch) => !batch.has("page"))).toBe(true);
		// A batch after the dismissals starts where the rows still active end, off a page's edge.
		expect(offsets.some((offset) => offset % size !== 0)).toBe(true);
	});
});

// Kills status controls drawn for a reader who cannot use them, the read-only
// line missing, and selection taken away along with them.
test.describe("a reader who cannot edit", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	test("sees no status controls and can still open alerts in Explore", async ({
		page,
	}) => {
		await page.goto(alerts(hashbrown.slug, "?status=all"));
		await expect(
			page.getByText("Read only. Ask a project Maintainer for access."),
		).toBeVisible();
		await expect(row(page, SHA256)).toContainText(/Dismissed /);
		await expect(page.getByRole("button", { name: /^Dismiss/ })).toHaveCount(0);
		await expect(page.getByRole("button", { name: /^Reactivate/ })).toHaveCount(
			0,
		);
		await row(page, BLAKE3).getByRole("checkbox").check();
		const open = page.getByRole("link", { name: "Open 1 in Explore" });
		const href = new URL(
			(await open.getAttribute("href")) ?? "",
			seed.console_url,
		);
		expect(href.pathname).toBe(nextPath(hashbrown.slug, "explore"));
		expect(href.searchParams.get("only")?.split(",")).toHaveLength(1);
	});
});

// Kills a contrast or structure failure in either theme, with a dimmed row and an open plot.
test("Alerts passes axe in both themes", async ({ page }) => {
	await axeInBothThemes(
		page,
		alerts(hashbrown.slug, "?status=all"),
		async () => {
			await expect(row(page, SHA256)).toContainText(/Dismissed /);
			await settle(page);
		},
	);
});

test.describe("on a phone", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills filters left in the toolbar, a window that is not one chip, a
	// status toggle folded away, groups that lose their checkboxes or bulk
	// actions, rows that do not fold, and targets under 44 px.
	test("filters sit behind a sheet, while status, groups, and bulk actions stay", async ({
		page,
	}) => {
		await page.goto(alerts(hashbrown.slug));
		await expect(status(page, "Dismissed")).toBeVisible();
		await expect(
			page.getByRole("button", { name: "Window 4w, 4 weeks" }),
		).toBeVisible();
		await expect(
			page.getByRole("button", { name: "Filter by branch, any" }),
		).toBeHidden();

		await page.getByRole("button", { name: "Filters", exact: true }).click();
		const sheet = page.getByRole("dialog", { name: "Filters" });
		await sheet.getByRole("button", { name: "Filter by branch, any" }).click();
		await sheet.getByRole("menuitemradio", { name: "main" }).click();
		await expect(page).toHaveURL(/[?&]branch=/);
		await sheet.getByRole("button", { name: "Done" }).click();
		await expect(
			page.getByRole("button", { name: "Filters (1)", exact: true }),
		).toBeVisible();

		const group = groupRow(page, MAIN_REPORT);
		const select = group.getByRole("checkbox");
		const blake3 = row(page, BLAKE3);
		await expect(blake3).toContainText("20.60 ns");
		for (const target of [
			select.locator(".."),
			blake3.getByRole("checkbox").locator(".."),
			blake3.getByRole("button", { name: `Expand ${BLAKE3}` }),
		]) {
			const box = await target.boundingBox();
			expect(box?.width).toBeGreaterThanOrEqual(44);
			expect(box?.height).toBeGreaterThanOrEqual(44);
		}
		await select.check();
		await expect(
			page.getByRole("button", { name: "Dismiss 1 selected active alert" }),
		).toBeVisible();
		await expect(
			page.getByRole("link", { name: "Open 1 in Explore" }),
		).toHaveText("Open 1");
		const width = await page.evaluate(
			() => document.documentElement.scrollWidth,
		);
		expect(width).toBeLessThanOrEqual(390);
	});
});
