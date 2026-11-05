import { consolePath, expect, seed, signedIn, test } from "./fixtures";

const { hashbrown } = seed.projects;

test.use({ storageState: signedIn(seed.member) });

/** 1,000 active keys, the names of keys 0 and 300 too long for their cell. */
const KEYS = Array.from({ length: 1000 }, (_, n) => ({
	uuid: `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`,
	project: "00000000-0000-4000-8000-00000000ffff",
	name: n === 0 || n === 300 ? `${"k".repeat(60)}-${n}` : `key-${String(n)}`,
	creation: new Date(Date.UTC(2026, 0, 1) - n * 60_000).toISOString(),
	expiration: "2027-10-02T10:00:00Z",
}));

// Kills a name opened far down the list whose close of the one kept above the
// screen moves the row pressed, or counts toward the page's layout shift
// score, which leaves out shifts within a moment of a press.
for (const width of [390, 768]) {
	test(`at ${width} px opening a name far down the keys closes the one above the screen and moves nothing`, async ({
		page,
	}) => {
		await page.setViewportSize({ width, height: 844 });
		await page.route(
			`${seed.api_url}/v0/projects/${hashbrown.slug}/keys?*`,
			async (route) => {
				if (route.request().method() !== "GET") {
					return route.continue();
				}
				const url = new URL(route.request().url());
				const size = Number(url.searchParams.get("per_page"));
				const at = Number(url.searchParams.get("page"));
				const list = url.searchParams.get("revoked") === "true" ? [] : KEYS;
				// The API's own answer, for its CORS headers.
				const response = await route.fetch();
				const { "content-length": _, ...headers } = response.headers();
				await route.fulfill({
					response,
					headers: { ...headers, "x-total-count": String(list.length) },
					json: list.slice((at - 1) * size, at * size),
				});
			},
		);
		await page.goto(consolePath(hashbrown.slug, "keys"));
		const table = page.getByRole("table", { name: "Active keys" });
		const reveal = (n: number) =>
			table.locator("button.ui-trunc", { hasText: `-${n}` });
		await reveal(0).click();
		await expect(reveal(0)).toHaveAttribute("aria-expanded", "true");
		for (let step = 0; step < 200; step++) {
			if (await reveal(300).isVisible()) {
				break;
			}
			await page.mouse.wheel(0, 300);
			await page.waitForTimeout(20);
		}
		await reveal(300).scrollIntoViewIfNeeded();
		// The wheel's last rows settle before the press.
		await page.waitForTimeout(1000);
		const row = reveal(300).locator("xpath=ancestor::tr");
		const before = (await row.boundingBox())?.y ?? 0;
		await page.evaluate(() => {
			(window as unknown as { score: number }).score = 0;
			new PerformanceObserver((list) => {
				for (const entry of list.getEntries()) {
					const shift = entry as unknown as {
						value: number;
						hadRecentInput: boolean;
					};
					if (!shift.hadRecentInput) {
						(window as unknown as { score: number }).score += shift.value;
					}
				}
			}).observe({ type: "layout-shift", buffered: false });
		});
		await reveal(300).click();
		await expect(reveal(300)).toHaveAttribute("aria-expanded", "true");
		await page.waitForTimeout(200);
		expect(Math.abs(((await row.boundingBox())?.y ?? 0) - before)).toBeLessThan(
			1,
		);
		expect(await table.locator("[data-open]").count()).toBe(1);
		expect(
			await page.evaluate(() => (window as unknown as { score: number }).score),
		).toBe(0);
	});
}
