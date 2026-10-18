import { describe, expect, test } from "vitest";
import type {
	JsonConsoleUser,
	JsonConsoleUserProject,
} from "../../types/bencher";
import type { Api } from "../api";
import { allowedQuery, organizationIn, withOrganization } from "./data";

const POMPEII = { uuid: "o1", name: "Pompeii LLC", slug: "pompeii-llc" };
const DINER = { uuid: "o2", name: "Bagge Diner", slug: "bagge-diner" };
const project = (slug: string, organization: string) =>
	({ slug, organization }) as JsonConsoleUserProject;
const USER = {
	uuid: "u",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	organizations: [POMPEII, DINER],
	projects: [project("hashbrown", "o1"), project("gizmo", "o2")],
} as JsonConsoleUser;

describe("organizationIn", () => {
	// Kills a page reached by UUID, as the classic console links it, that
	// finds no organization, and a lookup that takes another's.
	test("finds the organization by its slug or its UUID", () => {
		expect(organizationIn(USER, "bagge-diner")).toBe(DINER);
		expect(organizationIn(USER, "o2")).toBe(DINER);
		expect(organizationIn(USER, "o3")).toBeUndefined();
		expect(organizationIn(undefined, "o1")).toBeUndefined();
	});
});

describe("withOrganization", () => {
	// Kills a renamed organization the switcher keeps under its old slug, and
	// one that drops or renames another.
	test("swaps in the organization as the API now has it", () => {
		const renamed = { ...POMPEII, slug: "pompeii" };
		expect(withOrganization(USER, "o1", renamed)).toEqual({
			...USER,
			organizations: [renamed, DINER],
		});
	});

	// Kills a deleted organization, or its projects, left in the switcher, and
	// another organization's projects taken with it.
	test("drops a deleted organization and its projects", () => {
		expect(withOrganization(USER, "o1", undefined)).toEqual({
			...USER,
			organizations: [DINER],
			projects: [project("gizmo", "o2")],
		});
	});
});

describe("allowedQuery", () => {
	// Kills edit and delete read from the wrong permission, or swapped.
	test("reads edit and delete each from its own permission", async () => {
		const api = {
			get: async <T>(path: string) => ({
				data: { allowed: path.endsWith("/allowed/edit") } as T,
				headers: new Headers(),
			}),
		} as unknown as Api;
		expect(
			await allowedQuery(api, "pompeii-llc").queryFn({
				signal: new AbortController().signal,
			}),
		).toEqual({ edit: true, delete: false });
	});
});
