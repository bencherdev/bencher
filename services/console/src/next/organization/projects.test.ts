import { describe, expect, test } from "vitest";
import type { JsonConsoleUser, JsonProject } from "../../types/bencher";
import type { Api } from "../api";
import {
	endpointRows,
	organizationProjectsQuery,
	projectRows,
	projectSource,
} from "./projects";

const project = (
	uuid: string,
	name: string,
	organization: string,
	last_report?: number,
	active_alerts = 0,
) => ({
	uuid,
	name,
	slug: name.toLowerCase(),
	organization,
	bmf_version: 1 as const,
	...(last_report === undefined ? {} : { last_report }),
	active_alerts,
});

const BOOTSTRAP = {
	uuid: "u",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	organizations: [
		{ uuid: "o1", name: "Pompeii LLC", slug: "pompeii-llc" },
		{ uuid: "o2", name: "Bagge Diner", slug: "bagge-diner" },
	],
	projects: [
		project("p1", "Windmill", "o1", 1_000, 3),
		project("p2", "Cathode", "o2"),
		project("p3", "Gizmo", "o1"),
	],
} as JsonConsoleUser;

const listed = (uuid: string, name: string): JsonProject =>
	({
		uuid,
		organization: "o9",
		name,
		slug: name.toLowerCase(),
		visibility: "public",
		bmf_version: 1,
		created: "2026-09-01T00:00:00Z",
		modified: "2026-09-01T00:00:00Z",
	}) as JsonProject;

const signal = new AbortController().signal;

describe("projectRows", () => {
	// Kills another organization's project listed, the API's order kept, a
	// project with no runs given a time, or the alert count dropped.
	test("lists the organization's own projects by name, with their runs and alerts", () => {
		expect(projectRows(BOOTSTRAP, "o1")).toEqual([
			{
				uuid: "p3",
				name: "Gizmo",
				slug: "gizmo",
				last_report: null,
				active_alerts: 0,
			},
			{
				uuid: "p1",
				name: "Windmill",
				slug: "windmill",
				last_report: 1_000,
				active_alerts: 3,
			},
		]);
	});
});

describe("endpointRows", () => {
	// Kills the API's order kept, and runs or alerts made up for rows the
	// endpoint does not count.
	test("lists each project by name, with nothing it does not know", () => {
		expect(
			endpointRows([listed("p9", "Zebra"), listed("p8", "Anvil")]),
		).toEqual([
			{ uuid: "p8", name: "Anvil", slug: "anvil" },
			{ uuid: "p9", name: "Zebra", slug: "zebra" },
		]);
	});
});

describe("projectSource", () => {
	// Kills an endpoint read for an organization the bootstrap holds, by slug
	// or by UUID, a bootstrap read for one it lacks, and either chosen before
	// the bootstrap has loaded.
	test("reads the bootstrap for the reader's own organization, the endpoint for another", () => {
		expect(projectSource(undefined, "pompeii-llc")).toBeUndefined();
		for (const key of ["pompeii-llc", "o1"]) {
			expect(projectSource(BOOTSTRAP, key)).toEqual({
				from: "bootstrap",
				rows: projectRows(BOOTSTRAP, "o1"),
			});
		}
		expect(projectSource(BOOTSTRAP, "nowhere-labs")).toEqual({
			from: "endpoint",
		});
	});
});

describe("organizationProjectsQuery", () => {
	/** The API answering each page with `sizes`' rows, and refusing a page past them. */
	const pages = (sizes: number[], total: number) => {
		const paths: string[] = [];
		const api = {
			get: async <T>(path: string) => {
				const page = paths.length;
				paths.push(path);
				const size = sizes[page];
				if (size === undefined) {
					throw new Error(`read past the last page: ${path}`);
				}
				return {
					data: Array.from({ length: size }, (_, index) =>
						listed(`p${page}-${index}`, `P${page}-${index}`),
					) as T,
					headers: new Headers({ "X-Total-Count": String(total) }),
				};
			},
		} as unknown as Api;
		return { api, paths };
	};

	// Kills a list cut at the first page, pages asked for at another size, and
	// a page asked for past the API's total.
	test("reads every page at the API's largest size, and no further", async () => {
		const { api, paths } = pages([255, 255, 3], 513);
		const rows = await organizationProjectsQuery(api, "nowhere-labs").queryFn({
			signal,
		});
		expect(rows).toHaveLength(513);
		expect(paths).toEqual([
			"/v0/organizations/nowhere-labs/projects?per_page=255&page=1",
			"/v0/organizations/nowhere-labs/projects?per_page=255&page=2",
			"/v0/organizations/nowhere-labs/projects?per_page=255&page=3",
		]);
	});

	// Kills an extra page asked for when the total ends on a page's edge.
	test("stops once the total is in hand", async () => {
		const { api, paths } = pages([255], 255);
		await organizationProjectsQuery(api, "nowhere-labs").queryFn({ signal });
		expect(paths).toHaveLength(1);
	});

	// Kills a read that goes on past a short page because the API counts
	// more projects than its pages hold, as one made or deleted between two
	// pages leaves it.
	test("stops at a short page, whatever the total says", async () => {
		const { api, paths } = pages([255, 254], 510);
		const rows = await organizationProjectsQuery(api, "nowhere-labs").queryFn({
			signal,
		});
		expect(rows).toHaveLength(509);
		expect(paths).toHaveLength(2);
	});
});
