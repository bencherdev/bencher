import { randomUUID } from "node:crypto";
import type { APIRequestContext } from "@playwright/test";
import type {
	JsonOrganization,
	JsonPlot,
	JsonProject,
	JsonProjectKeyCreated,
	JsonReport,
} from "../src/types/bencher";
import { seed } from "./fixtures";

// Tests that change what they read make their own, since files run in
// parallel and every test may run more than once.

/** A name no other test, or another run of this one, uses. */
export const unique = (name: string) => `${name} ${randomUUID().slice(0, 8)}`;

const send = async <T>(
	request: APIRequestContext,
	method: "POST" | "PATCH",
	path: string,
	token: string,
	data: unknown,
): Promise<T> => {
	const response = await request.fetch(`${seed.api_url}${path}`, {
		method,
		headers: { Authorization: `Bearer ${token}` },
		data,
	});
	if (!response.ok()) {
		throw new Error(
			`${method} ${path} answered ${response.status()}: ${await response.text()}`,
		);
	}
	return (await response.json()) as T;
};

/** A version 1 project of the member's, in the seeded organization unless another is named. */
export const createProject = async (
	request: APIRequestContext,
	{ organization = seed.organization.slug }: { organization?: string } = {},
) => {
	const project = await send<JsonProject>(
		request,
		"POST",
		`/v0/organizations/${organization}/projects`,
		seed.member.token,
		{ name: unique("Rosti"), visibility: "public" },
	);
	return send<JsonProject>(
		request,
		"PATCH",
		`/v0/projects/${project.slug}`,
		seed.admin.token,
		{ bmf_version: 1 },
	);
};

/** An organization of the member's with no plan, so a private project is refused. */
export const createOrganization = (request: APIRequestContext) =>
	send<JsonOrganization>(
		request,
		"POST",
		"/v0/organizations",
		seed.member.token,
		{ name: unique("Bagge Farm") },
	);

export const createKey = (
	request: APIRequestContext,
	project: string,
	name: string,
) =>
	send<JsonProjectKeyCreated>(
		request,
		"POST",
		`/v0/projects/${project}/keys`,
		seed.member.token,
		{ name },
	);

/** The API's answer to the member reading a project. */
export const projectStatus = async (request: APIRequestContext, slug: string) =>
	(
		await request.get(`${seed.api_url}/v0/projects/${slug}`, {
			headers: { Authorization: `Bearer ${seed.member.token}` },
		})
	).status();

/** A report of `benchmarks` times `variants` lines of one measure, on main, an hour before the seed's now. */
export const createReport = (
	request: APIRequestContext,
	project: string,
	{ benchmarks, variants }: { benchmarks: number; variants: number },
) => {
	const name = (index: number) => `bench-${String(index).padStart(2, "0")}`;
	const results = Object.fromEntries(
		Array.from({ length: benchmarks }, (_, benchmark) => [
			name(benchmark),
			Array.from({ length: variants }, (_, n) => ({
				parameters: { n },
				measures: { latency: { value: 100 + n } },
			})),
		]),
	);
	const start = Date.parse(seed.now) - 60 * 60 * 1_000;
	return send<{ uuid: string }>(
		request,
		"POST",
		`/v0/projects/${project}/reports`,
		seed.member.token,
		{
			branch: "main",
			hash: "1e2e000000000000000000000000000000000000",
			testbed: "ubuntu-latest",
			start_time: new Date(start).toISOString(),
			end_time: new Date(start + 60_000).toISOString(),
			results: [JSON.stringify(results)],
			settings: { adapter: "json" },
		},
	);
};

/** A version 1 run of the member's, two minutes long from `start`. */
export const postReport = (
	request: APIRequestContext,
	project: string,
	{
		branch = "main",
		testbed = "ubuntu-latest",
		start,
		results,
	}: { branch?: string; testbed?: string; start: string; results: unknown },
) =>
	send<JsonReport>(
		request,
		"POST",
		`/v0/projects/${project}/reports`,
		seed.member.token,
		{
			branch,
			testbed,
			start_time: start,
			end_time: new Date(Date.parse(start) + 2 * 60 * 1000).toISOString(),
			results: [JSON.stringify(results)],
			settings: { adapter: "json" },
		},
	);

/** A project's pinned plots, top first. */
export const listPlots = async (request: APIRequestContext, project: string) =>
	(await (
		await request.get(`${seed.api_url}/v0/projects/${project}/plots`, {
			headers: { Authorization: `Bearer ${seed.member.token}` },
		})
	).json()) as JsonPlot[];

/** A threshold that alerts on any latency over 150, once a line has one value before. */
const STATIC_THRESHOLD = {
	models: [
		{
			measure: "latency",
			metric: "value",
			model: { test: "static", upper_boundary: 150 },
		},
	],
};

const benchmarks = (count: number) =>
	Array.from(
		{ length: count },
		(_, index) => `bench-${String(index).padStart(2, "0")}`,
	);

/** A run of `count` benchmarks on one branch, `high` of them over the threshold. */
const alertingReport = (
	branch: string,
	minutes: number,
	high: readonly string[],
	extra: Record<string, unknown> = {},
	count = 3,
) => {
	const start = Date.parse(seed.now) - minutes * 60 * 1_000;
	const results = Object.fromEntries(
		benchmarks(count).map((benchmark) => [
			benchmark,
			[
				{
					parameters: { n: 0 },
					measures: {
						latency: { value: high.includes(benchmark) ? 200 : 100 },
					},
				},
			],
		]),
	);
	return {
		branch,
		hash: `a1e2${String(minutes).padStart(36, "0")}`,
		testbed: "ubuntu-latest",
		start_time: new Date(start).toISOString(),
		end_time: new Date(start + 60_000).toISOString(),
		results: [JSON.stringify(results)],
		settings: { adapter: "json" },
		thresholds: STATIC_THRESHOLD,
		...extra,
	};
};

/**
 * A project of its own with three active alerts in two `main` reports, the
 * older raising two, and one alert on `feature` silenced when its head reset.
 */
export const createAlerts = async (request: APIRequestContext) => {
	const project = await createProject(request);
	const post = (body: unknown) =>
		send<{ uuid: string }>(
			request,
			"POST",
			`/v0/projects/${project.slug}/reports`,
			seed.member.token,
			body,
		);
	await post(alertingReport("main", 300, []));
	const older = await post(
		alertingReport("main", 240, ["bench-00", "bench-01"]),
	);
	const newer = await post(alertingReport("main", 180, ["bench-02"]));
	await post(alertingReport("feature", 120, []));
	const silenced = await post(alertingReport("feature", 60, ["bench-00"]));
	await post(
		alertingReport("feature", 30, [], { start_point: { reset: true } }),
	);
	return { project, older, newer, silenced };
};

/** One more `main` run of the project's, posted now, raising an alert on each of `high`. */
export const raiseAlerts = (
	request: APIRequestContext,
	slug: string,
	minutes: number,
	high: readonly string[],
) =>
	send<{ uuid: string }>(
		request,
		"POST",
		`/v0/projects/${slug}/reports`,
		seed.member.token,
		alertingReport("main", minutes, high),
	);

/** A project of its own whose one `main` report raised `count` alerts. */
export const createManyAlerts = async (
	request: APIRequestContext,
	count: number,
) => {
	const project = await createProject(request);
	const post = (body: unknown) =>
		send<{ uuid: string }>(
			request,
			"POST",
			`/v0/projects/${project.slug}/reports`,
			seed.member.token,
			body,
		);
	await post(alertingReport("main", 120, [], {}, count));
	await post(alertingReport("main", 60, benchmarks(count), {}, count));
	return project;
};
