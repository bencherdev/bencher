import { useLocation } from "@solidjs/router";
import { QueryClient } from "@tanstack/solid-query";
import { Show } from "solid-js";
import { afterEach, describe, expect, test } from "vitest";
import { page } from "vitest/browser";
import type {
	JsonConsoleUser,
	JsonOrganization,
	JsonProject,
	JsonUsage,
} from "../../types/bencher";
import { UsageKind } from "../../types/bencher";
import { ApiError } from "../api";
import { BOOTSTRAP, ORGANIZATION, mount } from "../organization/testing";
import { consoleUserQuery } from "../queries";
import { fakeApi } from "../settings/testing";
import { FLOOR_WIDTHS, STRESS, expectFoldAt, nameFloor } from "../ui/testing";
import NewProject from "./NewProject";
import OrgProjects from "./OrgProjects";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const HOUR = 60 * 60 * 1000;

/** Pompeii LLC with Windmill, run two hours ago with 3 active alerts, and Gizmo, never run. */
const MEMBER: JsonConsoleUser = {
	...BOOTSTRAP,
	projects: BOOTSTRAP.projects.map((project) =>
		project.slug === "windmill"
			? { ...project, last_report: Date.now() - 2 * HOUR, active_alerts: 3 }
			: project,
	),
};

/** Nowhere Labs, which the reader is not in. */
const ELSEWHERE: JsonOrganization = {
	...ORGANIZATION,
	uuid: "o9",
	name: "Nowhere Labs",
	slug: "nowhere-labs",
};

const listed = (name: string): JsonProject => ({
	uuid: `${name}-uuid`,
	organization: ELSEWHERE.uuid,
	name,
	slug: name.toLowerCase(),
	visibility: "public" as JsonProject["visibility"],
	bmf_version: 1,
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
});

type Answer = { data: unknown; headers?: Headers };

/** Projects at `key`, the API answering each read by `read`, by default never. */
const open = ({
	key = "pompeii-llc",
	bootstrap = MEMBER,
	organization = key === ELSEWHERE.slug ? ELSEWHERE : ORGANIZATION,
	read = () => new Promise(() => {}),
	client,
}: {
	key?: string;
	bootstrap?: JsonConsoleUser | null;
	organization?: JsonOrganization | null;
	read?: (path: string) => Promise<Answer>;
	client?: QueryClient;
} = {}) => {
	const reads: string[] = [];
	const { api } = fakeApi(undefined, async (path) => {
		reads.push(path);
		const { data, headers = new Headers() } = await read(path);
		return { data, headers } as never;
	});
	const mounted = mount(() => <OrgProjects />, {
		api,
		path: `/organizations/${key}/projects`,
		organization,
		allowed: null,
		bootstrap,
		...(client ? { client } : {}),
	});
	dispose = mounted.dispose;
	return { ...mounted, reads };
};

const ENDPOINT = "/v0/organizations/nowhere-labs/projects?per_page=255&page=1";

const table = () => page.getByRole("table", { name: "Projects" });
const rows = () =>
	[...document.querySelectorAll("tbody tr")].map((row) =>
		[...row.querySelectorAll("td")].map((cell) => cell.textContent?.trim()),
	);
const href = (name: string) =>
	page.getByRole("link", { name, exact: true }).element().getAttribute("href");

describe("a member's organization", () => {
	// Kills rows in the API's order, another organization's project listed, a
	// last run or alert count dropped or made up, and the endpoint read for
	// an organization the bootstrap already holds.
	test("lists its projects by name from the bootstrap, with their runs and alerts", async () => {
		const { reads } = open();
		await expect.element(table()).toBeVisible();
		expect(rows()).toEqual([
			["Gizmo", "No runs", "No alerts"],
			["Windmill", "2h ago", "3 alerts"],
		]);
		await expect.element(page.getByText("2 projects")).toBeVisible();
		expect(reads).toEqual([]);
	});

	// Kills a project leading anywhere but its Runs, alerts anywhere but its
	// Alerts, and New project for another organization.
	test("each project leads to its Runs, its alerts to its Alerts", async () => {
		open();
		await expect.element(table()).toBeVisible();
		expect(href("Gizmo")).toBe("/console/projects/gizmo/");
		expect(href("Windmill")).toBe("/console/projects/windmill/");
		expect(href("3 alerts")).toBe("/console/projects/windmill/alerts");
		expect(href("New project")).toBe(
			"/console/organizations/pompeii-llc/projects/add",
		);
	});

	// Kills an organization with no projects drawn as an empty table or a skeleton.
	test("with no projects says so", async () => {
		open({ bootstrap: { ...MEMBER, projects: [] } });
		await expect
			.element(page.getByRole("main").getByText("No projects"))
			.toBeVisible();
		await expect.element(page.getByText("0 projects")).toBeVisible();
		expect(table().query()).toBeNull();
	});

	// Kills a list that draws every row it holds, a row count without the
	// header row, and body rows numbered from the header's place.
	test("a long list draws only the rows on screen, numbered after the header", async () => {
		await page.viewport(1280, 800);
		open({
			bootstrap: {
				...MEMBER,
				projects: Array.from({ length: 300 }, (_, n) => ({
					...MEMBER.projects[0],
					uuid: `p${n}`,
					name: `Project ${String(n).padStart(3, "0")}`,
					slug: `project-${n}`,
				})) as JsonConsoleUser["projects"],
			},
		});
		await expect.element(table()).toBeVisible();
		await expect.element(page.getByText("300 projects")).toBeVisible();
		const drawn = document.querySelectorAll("tbody tr[aria-rowindex]");
		expect(drawn.length).toBeGreaterThan(0);
		expect(drawn.length).toBeLessThan(80);
		expect(table().element().getAttribute("aria-rowcount")).toBe("301");
		expect(drawn[0]?.getAttribute("aria-rowindex")).toBe("2");
		expect(drawn[0]?.textContent).toContain("Project 000");
	});

	// Kills a bootstrap restored from the browser, older than the reader's
	// membership, sending the organization's own list while its fresh copy is
	// on the way.
	test("a stale bootstrap without the organization waits for its fresh copy", async () => {
		let answer: (user: JsonConsoleUser) => void = () => {};
		const client = new QueryClient({
			defaultOptions: {
				queries: { retry: false, staleTime: Number.POSITIVE_INFINITY },
			},
		});
		const key = consoleUserQuery(fakeApi().api, "u").queryKey;
		client.setQueryData(key, {
			...MEMBER,
			organizations: MEMBER.organizations.filter(({ uuid }) => uuid !== "o1"),
		});
		client.invalidateQueries({ queryKey: key, refetchType: "none" });
		const { reads } = open({
			bootstrap: null,
			client,
			read: (path) =>
				path === "/v0/console/users/u"
					? new Promise((resolve) => {
							answer = (user) => resolve({ data: user });
						})
					: new Promise(() => {}),
		});
		await expect.poll(() => reads).toEqual(["/v0/console/users/u"]);
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(reads).toEqual(["/v0/console/users/u"]);

		answer(MEMBER);
		await expect.element(table()).toBeVisible();
		expect(rows().map(([name]) => name)).toEqual(["Gizmo", "Windmill"]);
		expect(reads).toEqual(["/v0/console/users/u"]);
	});

	// Kills a failed bootstrap that leaves a skeleton forever, and a Retry
	// that does not ask again.
	test("a failed load says so and retries", async () => {
		let fail = true;
		const { reads } = open({
			bootstrap: null,
			read: async () => {
				if (fail) {
					throw new ApiError(undefined, "network", "offline");
				}
				return { data: MEMBER };
			},
		});
		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("Projects did not load");
		fail = false;
		await page.getByRole("button", { name: "Retry" }).click();

		await expect.element(table()).toBeVisible();
		expect(reads).toEqual(["/v0/console/users/u", "/v0/console/users/u"]);
	});

	// Kills a list that keeps its columns where a project's name, beside a
	// five digit alert count, falls under 16 characters or scrolls sideways,
	// and one that folds late.
	test("holds 16 characters of name, or folds", async () => {
		const long = {
			...MEMBER,
			projects: [
				{
					...MEMBER.projects[0],
					name: STRESS.name,
					last_report: Date.now() - 330 * 24 * HOUR,
					active_alerts: 99_999,
				},
			],
		} as JsonConsoleUser;
		const held: Record<number, unknown> = {};
		for (const width of FLOOR_WIDTHS) {
			await page.viewport(width, 720);
			open({ bootstrap: long });
			await expect.element(table()).toBeVisible();
			await document.fonts.ready;
			held[width] = nameFloor(table().element(), "Project");
			if (width === 1440) {
				expectFoldAt(table().element(), "Project", 328);
			}
			dispose?.();
			dispose = undefined;
		}
		const fine = { name: "held", sideways: false };
		expect(held).toEqual({
			320: { name: "folded", sideways: false },
			768: fine,
			1024: fine,
			1440: fine,
		});
	});
});

describe("an organization the reader is not in", () => {
	// Kills a server admin's list drawn from the bootstrap (empty), the
	// endpoint read a page short or at another size, rows out of name order,
	// runs or alerts made up for them, and New project offered where its
	// page would not load.
	test("a server admin reads its projects from the API, with no runs or alerts", async () => {
		const { reads } = open({
			key: ELSEWHERE.slug,
			read: async () => ({
				data: [listed("Zebra"), listed("Anvil")],
				headers: new Headers({ "X-Total-Count": "2" }),
			}),
		});
		await expect.element(table()).toBeVisible();
		expect(rows()).toEqual([
			["Anvil", "", ""],
			["Zebra", "", ""],
		]);
		expect(href("Anvil")).toBe("/console/projects/anvil/");
		await expect.element(page.getByText("2 projects")).toBeVisible();
		expect(page.getByRole("link", { name: "New project" }).query()).toBeNull();
		expect(reads).toEqual([ENDPOINT]);
	});

	// Kills the endpoint asked before the organization has answered, which
	// would send a second doomed request for a reader the API refuses.
	test("asks the API only once the organization has answered", async () => {
		const { reads } = open({ key: ELSEWHERE.slug, organization: null });
		await expect.poll(() => reads).toEqual(["/v0/organizations/nowhere-labs"]);
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(reads).toEqual(["/v0/organizations/nowhere-labs"]);
	});

	// Kills a refusal drawn as a failure to retry, and the endpoint asked
	// after the API refused the organization itself.
	test.each([
		[403, "forbidden"],
		[404, "not_found"],
	] as const)(
		"a reader the API refuses (%i) is told it is not found, with no Retry",
		async (status, kind) => {
			const { reads } = open({
				key: ELSEWHERE.slug,
				organization: null,
				read: async () => {
					throw new ApiError(status, kind, "{}");
				},
			});
			await expect
				.element(page.getByRole("main").getByText("Organization not found"))
				.toBeVisible();
			expect(page.getByRole("alert").query()).toBeNull();
			expect(page.getByRole("button", { name: "Retry" }).query()).toBeNull();
			expect(reads).toEqual(["/v0/organizations/nowhere-labs"]);
		},
	);

	// Kills a refusal of the list itself drawn as a failure to retry.
	test("a list the API refuses is not found too", async () => {
		open({
			key: ELSEWHERE.slug,
			read: async () => {
				throw new ApiError(403, "forbidden", "{}");
			},
		});
		await expect
			.element(page.getByRole("main").getByText("Organization not found"))
			.toBeVisible();
		expect(page.getByRole("button", { name: "Retry" }).query()).toBeNull();
	});

	// Kills a server failure drawn as a refusal, and a Retry that does not ask
	// the list again.
	test("a list the API fails to answer says so and retries", async () => {
		let fail = true;
		const { reads } = open({
			key: ELSEWHERE.slug,
			read: async () => {
				if (fail) {
					throw new ApiError(500, "server", "{}");
				}
				return { data: [listed("Anvil")] };
			},
		});
		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("Projects did not load");
		fail = false;
		await page.getByRole("button", { name: "Retry" }).click();
		await expect.element(table()).toBeVisible();
		expect(reads).toEqual([ENDPOINT, ENDPOINT]);
	});
});

// Kills a rail that leaves Projects out, puts it anywhere but third, marks
// another section current, or leads anywhere but here.
test("the rail has Projects third, current", async () => {
	await page.viewport(1280, 800);
	open();
	const rail = page.getByRole("navigation", {
		name: "Organization settings",
		exact: true,
	});
	await expect.element(rail).toBeVisible();
	expect(
		rail
			.getByRole("link")
			.elements()
			.map((link) => link.textContent),
	).toEqual(["General", "Members", "Projects", "Billing"]);
	const projects = rail.getByRole("link", { name: "Projects" });
	await expect.element(projects).toHaveAttribute("aria-current", "page");
	await expect
		.element(projects)
		.toHaveAttribute("href", "/console/organizations/pompeii-llc/projects");
});

// Kills a list kept apart from the bootstrap New project writes to, which
// would leave the new project out until the next read.
test("a project made in New project is in the list at once", async () => {
	const usage: Partial<JsonUsage> = { kind: UsageKind.SelfHostedFree };
	const { api } = fakeApi(
		async ({ body }) =>
			({
				...listed((body as { name: string }).name),
				uuid: "p9",
				organization: ORGANIZATION.uuid,
				slug: "hash-brown",
			}) satisfies JsonProject,
		async () => ({ data: usage, headers: new Headers() }) as never,
	);
	const mounted = mount(
		() => {
			const location = useLocation();
			return (
				<Show
					when={location.pathname.endsWith("/projects/add")}
					fallback={<OrgProjects />}
				>
					<NewProject />
				</Show>
			);
		},
		{
			api,
			path: "/organizations/pompeii-llc/projects/add",
			allowed: null,
			bootstrap: MEMBER,
		},
	);
	dispose = mounted.dispose;
	await page.getByRole("textbox", { name: "Name" }).fill("Hash Brown");
	await page.getByRole("button", { name: "Create project" }).click();
	await expect
		.poll(() => mounted.history.get())
		.toBe("/console/projects/hash-brown/");

	mounted.history.set({ value: "/organizations/pompeii-llc/projects" });
	await expect.element(table()).toBeVisible();
	expect(rows().map(([name]) => name)).toEqual([
		"Gizmo",
		"Hash Brown",
		"Windmill",
	]);
	expect(href("Hash Brown")).toBe("/console/projects/hash-brown/");
});
