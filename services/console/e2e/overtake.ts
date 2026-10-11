import type { APIRequestContext, Page } from "@playwright/test";
import type { JsonConsoleAlerts } from "../src/types/bencher";
import { expect, seed, settle } from "./fixtures";

/** The name each row of the alerts made by `createManyAlerts` reads, in order. */
export const manyAlertNames = (count: number) =>
	Array.from(
		{ length: count },
		(_, index) => `bench-${String(index).padStart(2, "0")} n=0 Latency`,
	);

/**
 * Open `path`, hold the next batch of its alerts at the route, dismiss the
 * first alert, and let that batch go once the API took the dismissal: the API
 * reads the batch from an offset the dismissal moved. Resolves to the offset
 * of every batch asked for.
 */
export const dismissWhileNextBatchIsOut = async (
	page: Page,
	path: string,
	label: string,
) => {
	const table = page.getByRole("table", { name: label, exact: true });
	const toTable = () =>
		page.evaluate((name) => {
			document
				.querySelector(`table[aria-label="${name}"]`)
				?.scrollIntoView({ block: "start", behavior: "instant" });
		}, label);
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const offsets: number[] = [];
	await page.route(
		(url) => /\/v0\/console\/projects\/[^/]+\/alerts$/.test(url.pathname),
		async (route) => {
			if (route.request().method() === "GET") {
				const offset = Number(
					new URL(route.request().url()).searchParams.get("offset"),
				);
				offsets.push(offset);
				if (offset > 0) {
					await held;
				}
			}
			await route.continue();
		},
	);
	await page.goto(path);
	await expect(table).toBeVisible();
	await toTable();
	const first = table.getByRole("button", {
		name: "Dismiss the alert on bench-00 n=0 Latency",
	});
	await expect(first).toBeVisible();
	await expect(async () => {
		if (offsets.length < 2) {
			await page.mouse.wheel(0, 2_000);
		}
		expect(offsets.length).toBeGreaterThan(1);
	}).toPass();
	await toTable();
	const taken = page.waitForResponse(
		(response) =>
			response.request().method() === "PATCH" &&
			/\/v0\/console\/projects\/[^/]+\/alerts$/.test(response.url()),
	);
	await first.click();
	expect((await taken).ok()).toBe(true);
	release();
	return offsets;
};

/**
 * Open `path`, dismiss the first three alerts, and load three batches; return
 * to the window; then have another reader dismiss bench-03 and dismiss it here
 * too, which the API takes as no change, so the page reads the list again.
 * That reread's first batch is held until nothing else was asked for.
 * Resolves to the batch size, the offsets asked for on returning to the
 * window, and those asked for since the reread, in order.
 */
export const refocusThenReread = async (
	page: Page,
	request: APIRequestContext,
	slug: string,
	path: string,
	label: string,
) => {
	const table = page.getByRole("table", { name: label, exact: true });
	const toTable = () =>
		page.evaluate((name) => {
			document
				.querySelector(`table[aria-label="${name}"]`)
				?.scrollIntoView({ block: "start", behavior: "instant" });
		}, label);
	const dismiss = (name: string) =>
		table.getByRole("button", { name: `Dismiss the alert on ${name}` });
	const patched = () =>
		page.waitForResponse(
			(response) =>
				response.request().method() === "PATCH" &&
				/\/v0\/console\/projects\/[^/]+\/alerts$/.test(response.url()),
		);
	const offsets: number[] = [];
	let size = 0;
	const alerts = new Map<string, string>();
	let holding = false;
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route(
		(url) => /\/v0\/console\/projects\/[^/]+\/alerts$/.test(url.pathname),
		async (route) => {
			if (route.request().method() !== "GET") {
				await route.continue();
				return;
			}
			const params = new URL(route.request().url()).searchParams;
			const offset = Number(params.get("offset"));
			offsets.push(offset);
			size = Number(params.get("per_page"));
			if (holding && offset === 0) {
				await held;
			}
			const response = await route.fetch();
			const list = (await response.json()) as JsonConsoleAlerts;
			for (const group of list.groups) {
				for (const { line } of group.alerts) {
					const name = list.benchmarks[line.benchmark]?.name;
					if (name && line.alert) {
						alerts.set(name, line.alert.uuid);
					}
				}
			}
			await route.fulfill({ response });
		},
	);
	await page.goto(path);
	await expect(table).toBeVisible();
	await toTable();
	for (const name of manyAlertNames(3)) {
		const taken = patched();
		await dismiss(name).click();
		expect((await taken).ok()).toBe(true);
	}
	await expect(async () => {
		if (offsets.length < 3) {
			await page.mouse.wheel(0, 2_000);
		}
		expect(offsets.length).toBeGreaterThan(2);
	}).toPass();
	await settle(page);

	const returned = offsets.length;
	await page.evaluate(() =>
		window.dispatchEvent(new Event("visibilitychange")),
	);
	await expect.poll(() => offsets.length).toBeGreaterThan(returned);
	await settle(page);
	const focus = offsets.slice(returned);

	const elsewhere = await request.fetch(
		`${seed.api_url}/v0/console/projects/${slug}/alerts`,
		{
			method: "PATCH",
			headers: { Authorization: `Bearer ${seed.member.token}` },
			data: { status: "dismissed", alerts: [alerts.get("bench-03")] },
		},
	);
	expect(elsewhere.ok()).toBe(true);
	holding = true;
	const asked = offsets.length;
	await toTable();
	const taken = patched();
	await dismiss("bench-03 n=0 Latency").click();
	expect((await taken).ok()).toBe(true);
	await expect(
		page.getByText("Some alerts did not change, so the list is read again."),
	).toBeVisible();
	await expect.poll(() => offsets.length).toBeGreaterThan(asked);
	// Nothing else is asked for while the first batch is out.
	await page.waitForTimeout(500);
	expect.soft(offsets.slice(asked)).toEqual([0]);
	holding = false;
	release();
	await expect.poll(() => offsets.length).toBeGreaterThan(asked + 1);
	await settle(page);
	return { size, focus, reread: () => offsets.slice(asked) };
};

/** Each alert row's name in the table labelled `label`, top to bottom, once every batch has loaded. */
export const alertsTopToBottom = async (
	page: Page,
	label: string,
	last: string,
) => {
	const named = (rows: string) =>
		page.evaluate(
			async ([name, which]) => {
				const names = new Map<number, string>();
				const frame = () =>
					new Promise((resolve) =>
						requestAnimationFrame(() => requestAnimationFrame(resolve)),
					);
				const read = () => {
					const list = [...document.querySelectorAll("table")].find(
						(each) => each.getAttribute("aria-label") === name,
					);
					for (const row of list?.querySelectorAll("tbody tr[aria-rowindex]") ??
						[]) {
						const expand = row
							.querySelector("button[aria-label^='Expand ']")
							?.getAttribute("aria-label");
						if (expand) {
							names.set(
								Number(row.getAttribute("aria-rowindex")),
								expand.slice("Expand ".length),
							);
						}
					}
				};
				if (which === "last") {
					window.scrollTo({
						top: document.documentElement.scrollHeight,
						behavior: "instant",
					});
					await frame();
					read();
				} else {
					for (
						let top = 0;
						top <= document.documentElement.scrollHeight;
						top += window.innerHeight / 2
					) {
						window.scrollTo({ top, behavior: "instant" });
						await frame();
						read();
					}
				}
				return [...names].sort(([a], [b]) => a - b).map(([, each]) => each);
			},
			[label, rows] as const,
		);
	await expect(async () => {
		await page.mouse.wheel(0, 5_000);
		expect((await named("last")).at(-1)).toBe(last);
	}).toPass();
	await settle(page);
	return named("all");
};
