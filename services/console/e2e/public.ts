import type { APIRequestContext, Page } from "@playwright/test";
import { createProject, postReport } from "./api";
import { dimensions } from "./explore";
import { expect } from "./fixtures";

export const DAY = 24 * 60 * 60 * 1000;

export const publicPath = (slug: string, search = "") =>
	`/next/perf/${slug}${search}`;

/** A project of two lines that reported in the last two days: the API reaches back three months from its own clock for a reader who is not signed in. */
export const twoRecentLines = (request: APIRequestContext) =>
	recentLines(request, 2);

/** A project of `count` lines, one per size, that reported in the last two days. */
export const recentLines = async (
	request: APIRequestContext,
	count: number,
) => {
	const project = await createProject(request);
	for (const daysAgo of [2, 1]) {
		await postReport(request, project.slug, {
			start: new Date(Date.now() - daysAgo * DAY).toISOString(),
			results: {
				hash: Array.from({ length: count }, (_, index) => ({
					parameters: { size: index + 1 },
					measures: { latency: { value: index + 1 } },
				})),
			},
		});
	}
	const id = await dimensions(request, project.slug);
	const search = `?${new URLSearchParams({
		branches: id.branch("main"),
		testbeds: id.testbed("ubuntu-latest"),
		benchmarks: id.benchmark("hash"),
		measures: id.measure("Latency"),
	})}`;
	return { project, search };
};

export const twoLinesDrawn = (page: Page) =>
	expect(page.getByRole("figure")).toHaveAccessibleName("2 lines");
