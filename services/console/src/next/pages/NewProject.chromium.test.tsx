import { afterEach, describe, expect, test } from "vitest";
import { page } from "vitest/browser";
import {
	type JsonConsoleUser,
	type JsonProject,
	type JsonUsage,
	UsageKind,
} from "../../types/bencher";
import { ApiError } from "../api";
import { BOOTSTRAP, mount } from "../organization/testing";
import { consoleUserQuery } from "../queries";
import { fakeApi } from "../settings/testing";
import NewProject from "./NewProject";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const made = (body: unknown, bmf_version: number): JsonProject => ({
	uuid: "p9",
	organization: "o1",
	name: (body as { name: string }).name,
	slug: "hash-brown-x1",
	visibility: "public" as JsonProject["visibility"],
	bmf_version,
	created: "2026-10-17T00:00:00Z",
	modified: "2026-10-17T00:00:00Z",
});

/** Each organization's usage: Pompeii LLC holds a license with room, Bagge Diner nothing. */
const USAGE: Record<string, Partial<JsonUsage>> = {
	"pompeii-llc": {
		kind: UsageKind.SelfHostedLicensed,
		license: { entitlements: 1000 } as NonNullable<JsonUsage["license"]>,
		metrics: 10,
	},
	"bagge-diner": { kind: UsageKind.SelfHostedFree },
};

/**
 * New project for Pompeii LLC; the API makes each project at `bmf_version`
 * unless `answer` says otherwise, and `steps` records each flush of the
 * browser's store.
 */
const open = ({
	bmf_version = 0,
	answer,
	path = "/organizations/pompeii-llc/projects/add",
}: {
	bmf_version?: number;
	answer?: (body: unknown) => Promise<JsonProject>;
	path?: string;
} = {}) => {
	const { api, sent } = fakeApi(
		async ({ body }) => (answer ? answer(body) : made(body, bmf_version)),
		async (path) => {
			const organization = path.split("/")[3] ?? "";
			const usage = USAGE[organization];
			if (!usage) {
				throw new ApiError(404, "not_found", "{}");
			}
			return { data: usage, headers: new Headers() } as never;
		},
	);
	const steps: string[] = [];
	const bootstrap = () =>
		mounted.client.getQueryData<JsonConsoleUser>(
			consoleUserQuery(api, "u").queryKey,
		);
	const mounted = mount(() => <NewProject />, {
		api,
		path,
		allowed: null,
		flush: async () => {
			// A store that takes a moment, so a page that does not wait for it shows.
			await new Promise((resolve) => setTimeout(resolve, 20));
			const project = bootstrap()?.projects.find(
				({ slug }) => slug === "hash-brown-x1",
			);
			steps.push(`flush ${project?.bmf_version}`);
		},
	});
	dispose = mounted.dispose;
	return { ...mounted, sent, steps, bootstrap };
};

const textbox = (name: string) => page.getByRole("textbox", { name });
const create = () => page.getByRole("button", { name: "Create project" });
const radio = (name: string) => page.getByRole("radio", { name });
const organization = () => page.getByRole("combobox", { name: "Organization" });

describe("the form", () => {
	// Kills a slug not derived from the name, a request to another
	// organization, and one that sends an untrimmed name, a slug, an empty
	// URL, a public visibility, or a version the API decides on its own.
	test("derives the slug and sends only the name, the URL, and private", async () => {
		const { sent } = open();
		await textbox("Name").fill(" Hash Brown ");
		await expect
			.element(page.getByText("hash-brown", { exact: true }))
			.toBeVisible();
		await create().click();
		await expect.poll(() => sent.length).toBe(1);

		dispose?.();
		const second = open();
		await textbox("Name").fill("Hash Brown");
		await textbox("URL").fill("https://hash.example");
		await expect.element(radio("Private")).toBeEnabled();
		await radio("Private").click();
		await create().click();
		await expect.poll(() => second.sent.length).toBe(1);

		expect(sent).toEqual([
			{
				method: "POST",
				path: "/v0/organizations/o1/projects",
				body: { name: "Hash Brown" },
			},
		]);
		expect(second.sent[0]?.body).toEqual({
			name: "Hash Brown",
			url: "https://hash.example",
			visibility: "private",
		});
	});

	// Kills a preset that matches the path only by slug, as the switcher's
	// footer opens it by UUID.
	test("opened by the organization's UUID, presets that organization", async () => {
		open({ path: "/organizations/o1/projects/add" });
		await expect.element(organization()).toHaveValue("o1");
	});

	// Kills Create offered before there is a name, or past the API's 64 bytes.
	test("holds Create back until the name is good", async () => {
		open();
		await expect.element(create()).toBeDisabled();
		await textbox("Name").fill("x".repeat(65));
		await expect.element(page.getByText("65 of 64 bytes")).toBeVisible();
		await expect.element(create()).toBeDisabled();
		await textbox("Name").fill("Hash Brown");
		await expect.element(create()).toBeEnabled();
	});

	// Kills an organization preset to any but the one it opened for (here not
	// the first by name), Private offered to an organization whose plan
	// refuses it, a Private choice carried to such an organization, a draft
	// lost when the organization changes, and a change that stays on the old
	// organization.
	test("Private follows the organization's plan", async () => {
		const { sent, history } = open();
		await expect.element(organization()).toHaveValue("o1");
		await textbox("Name").fill("Hash Brown");
		await expect.element(radio("Private")).toBeEnabled();
		await radio("Private").click();
		await expect.element(radio("Private")).toBeChecked();

		await organization().selectOptions("Bagge Diner");

		await expect
			.poll(() => history.get())
			.toBe("/organizations/bagge-diner/projects/add");
		await expect.element(organization()).toHaveValue("o2");
		await expect.element(radio("Private")).toBeDisabled();
		await expect.element(radio("Public")).toBeChecked();
		await expect.element(textbox("Name")).toHaveValue("Hash Brown");
		await create().click();
		await expect.poll(() => sent.length).toBe(1);
		expect(sent[0]).toEqual({
			method: "POST",
			path: "/v0/organizations/o2/projects",
			body: { name: "Hash Brown" },
		});
	});

	// Kills a second POST from a second press while the first is in flight.
	test("Create holds while the project is being made", async () => {
		let finish: (project: JsonProject) => void = () => {};
		const { sent } = open({
			answer: () =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		});
		await textbox("Name").fill("Hash Brown");
		await create().click();

		await expect.element(create()).toBeDisabled();
		(create().element() as HTMLButtonElement).click();
		document.querySelector("form")?.requestSubmit();
		expect(sent).toHaveLength(1);
		finish(made({ name: "Hash Brown" }, 0));
	});

	// Kills a refusal that leaves anyway, says nothing, hides the API's
	// reason, throws the draft away, or keeps Create held.
	test("a refusal says why and keeps the draft", async () => {
		const { steps } = open({
			answer: async () => {
				throw new ApiError(402, "client", '{"message":"Private needs a plan"}');
			},
		});
		await textbox("Name").fill("Hash Brown");
		await textbox("URL").fill("https://hash.example");
		await create().click();

		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent(
				"Bencher did not make the project: Private needs a plan",
			);
		expect(steps).toEqual([]);
		await expect.element(textbox("Name")).toHaveValue("Hash Brown");
		await expect.element(textbox("URL")).toHaveValue("https://hash.example");
		await expect.element(create()).toBeEnabled();
	});
});

describe("after create", () => {
	// Kills a bootstrap left without the project, one that guesses its
	// version, a project at either version sent anywhere but its root in the
	// console, and one moved before the bootstrap holds it.
	test.each([0, 1])(
		"a version %i project joins the bootstrap, then lands on its root",
		async (bmf_version) => {
			const { steps, bootstrap, history } = open({ bmf_version });
			await textbox("Name").fill("Hash Brown");
			await create().click();

			await expect
				.poll(() => history.get())
				.toBe("/console/projects/hash-brown-x1/");
			expect(steps).toEqual([`flush ${bmf_version}`]);
			expect(bootstrap()?.projects).toEqual([
				...BOOTSTRAP.projects,
				{
					uuid: "p9",
					name: "Hash Brown",
					slug: "hash-brown-x1",
					organization: "o1",
					bmf_version,
					active_alerts: 0,
				},
			]);
		},
	);
});
