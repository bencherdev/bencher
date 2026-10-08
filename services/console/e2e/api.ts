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
