import { randomUUID } from "node:crypto";
import type { APIRequestContext } from "@playwright/test";
import type {
	JsonOrganization,
	JsonProject,
	JsonProjectKeyCreated,
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
