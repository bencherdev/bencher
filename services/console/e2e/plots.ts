import type { APIRequestContext } from "@playwright/test";
import type { JsonPlot } from "../src/types/bencher";
import { createProject, postReport } from "./api";
import { dimensions } from "./explore";
import { seed } from "./fixtures";

const DAY = 24 * 60 * 60;

/** Pin a plot of `hash` at the top of a project's Plots. */
export const createPin = async (
	request: APIRequestContext,
	project: string,
	{ title, days = 28 }: { title: string; days?: number },
	id?: Awaited<ReturnType<typeof dimensions>>,
) => {
	const named = id ?? (await dimensions(request, project));
	const response = await request.post(
		`${seed.api_url}/v0/projects/${project}/plots`,
		{
			headers: { Authorization: `Bearer ${seed.member.token}` },
			data: {
				title,
				lower_value: false,
				upper_value: false,
				lower_boundary: false,
				upper_boundary: false,
				x_axis: "date_time",
				window: days * DAY,
				branches: [named.branch("main")],
				testbeds: [named.testbed("ubuntu-latest")],
				benchmarks: [named.benchmark("hash")],
				measures: [named.measure("Latency")],
			},
		},
	);
	if (!response.ok()) {
		throw new Error(
			`Pin ${title}: ${response.status()} ${await response.text()}`,
		);
	}
	return (await response.json()) as JsonPlot;
};

/** A project of the member's with a week of runs of `hash` and `count` pins, "Pin 1" on top. */
export const pinnedProject = async (request: APIRequestContext, count = 3) => {
	const project = await createProject(request);
	for (const day of [8, 10, 12]) {
		await postReport(request, project.slug, {
			start: `2026-09-${String(day).padStart(2, "0")}T12:00:00Z`,
			results: {
				hash: [
					{ parameters: { size: 1 }, measures: { latency: { value: day } } },
					{
						parameters: { size: 2 },
						measures: { latency: { value: 2 * day } },
					},
				],
			},
		});
	}
	const id = await dimensions(request, project.slug);
	for (let index = count; index > 0; index--) {
		await createPin(request, project.slug, { title: `Pin ${index}` }, id);
	}
	return project;
};
