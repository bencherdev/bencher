import type { APIRequestContext, Locator, Page } from "@playwright/test";
import {
	type ExploreQuery,
	blankQuery,
	encodeQuery,
} from "../src/next/query/query";
import { expect, consolePath, seed } from "./fixtures";

const { hashbrown } = seed.projects;

type Dimension = "branches" | "testbeds" | "benchmarks" | "measures";

/** Each dimension's UUIDs by name, as the API lists them. */
export const dimensions = async (
	request: APIRequestContext,
	project: string,
) => {
	const read = async (dimension: Dimension) => {
		const response = await request.get(
			`${seed.api_url}/v0/projects/${project}/${dimension}?per_page=64`,
			{ headers: { Authorization: `Bearer ${seed.member.token}` } },
		);
		const list = (await response.json()) as { uuid: string; name: string }[];
		return (name: string) => {
			const found = list.find((entry) => entry.name === name);
			if (!found) {
				throw new Error(`No ${dimension} named ${name}`);
			}
			return found.uuid;
		};
	};
	return {
		branch: await read("branches"),
		testbed: await read("testbeds"),
		benchmark: await read("benchmarks"),
		measure: await read("measures"),
	};
};

export const explore = (project: string, query?: Partial<ExploreQuery>) =>
	`${consolePath(project, "explore")}${query ? encodeQuery({ ...blankQuery(), ...query }) : ""}`;

/** The seed's alerting variant and the one beside it: blake3 at 64 KiB on one thread. */
export const twoLines = async (request: APIRequestContext) => {
	const id = await dimensions(request, hashbrown.slug);
	return {
		id,
		query: {
			branches: [{ uuid: id.branch("main") }],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: [id.benchmark("blake3")],
			sets: [{ input_bytes: 65536, threads: 1 }],
			measures: [id.measure("Latency")],
			metrics: ["value"],
		},
	};
};

/**
 * Scroll to the page's end: where the plot's top sits then, and the room left
 * between the plot's bottom and the page's end, beyond the page's own padding.
 */
export const plotAtEnd = async (page: Page) => {
	await page.evaluate(() =>
		window.scrollTo({
			top: document.documentElement.scrollHeight,
			behavior: "instant",
		}),
	);
	return page.getByRole("figure").evaluate((figure) => {
		const box = figure.getBoundingClientRect();
		const main = figure.closest("main");
		const end = document.documentElement.scrollHeight - window.scrollY;
		const padding = main
			? Number.parseFloat(getComputedStyle(main).paddingBottom)
			: 0;
		return { top: box.top, below: end - box.bottom - padding };
	});
};

/** Asserts that a box's value shows `name` over `units`, neither cut short. */
export const expectNameOverUnits = async (
	box: Locator,
	name: string,
	units: string,
) => {
	const top = box.getByText(name, { exact: true });
	const bottom = box.getByText(units, { exact: true });
	await expect(bottom).toBeVisible();
	for (const line of [top, bottom]) {
		expect(
			await line.evaluate(
				(element) => element.scrollWidth <= element.clientWidth,
			),
		).toBe(true);
	}
	const above = await top.boundingBox();
	const below = await bottom.boundingBox();
	expect(below?.y ?? 0).toBeGreaterThanOrEqual(
		(above?.y ?? 0) + (above?.height ?? 0),
	);
};

/** The first report in view, as Home on the plot area named `name` reads it out. */
export const firstInView = async (page: Page, name: RegExp) => {
	// Away from the plot, so no hover names a report of its own.
	await page.mouse.move(0, 0);
	await page.getByRole("group", { name }).focus();
	await page.keyboard.press("Home");
	const status = page.getByRole("figure").getByRole("status");
	await expect(status).toContainText(/\d/);
	const text = await status.textContent();
	await page.keyboard.press("Escape");
	return text;
};

/** A mouse's drag across the plot area named `name`, from and to fractions of its width, which zooms it. */
export const dragAcross = async (
	page: Page,
	name: RegExp,
	from = 0.3,
	to = 0.7,
) => {
	const area = page.getByRole("group", { name });
	await area.scrollIntoViewIfNeeded();
	const box = await area.boundingBox();
	if (!box) {
		throw new Error("No plot area");
	}
	const y = box.y + box.height - 10;
	await page.mouse.move(box.x + box.width * from, y);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width * to, y, { steps: 4 });
	await page.mouse.up();
};

/** Asserts that the page shows no zoom control and names no zoomed span. */
export const expectNoZoomReadout = async (page: Page) => {
	await expect(page.getByRole("group", { name: "Zoom" })).toHaveCount(0);
	await expect(page.getByRole("button", { name: /zoom/i })).toHaveCount(0);
	await expect(page.getByText(/^\w{3} \d+(, [\d:]+)? to /)).toHaveCount(0);
};
