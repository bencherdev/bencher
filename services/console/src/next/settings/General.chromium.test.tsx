import type { JsonConsoleProject } from "../../types/bencher";
import { afterEach, describe, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import { ApiError } from "../api";
import { VERSION_KEY } from "../memory";
import General from "./General";
import { PROJECT, bootstrapOf, fakeApi, mount, watchFallback } from "./testing";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const project = (client: ReturnType<typeof mount>["client"]) =>
	client.getQueryData<JsonConsoleProject>(["console", "project", "hashbrown"])
		?.project;

const save = () => page.getByRole("button", { name: "Save changes" });

describe("a save", () => {
	// Kills a save that waits for the API before the project changes, an undo
	// that keeps the refused change, a refusal shown away from Visibility, and
	// a form that drops the reader's edits when it is refused.
	test("shows at once, and a refusal puts the project back and says why in place", async () => {
		let refuse: (error: unknown) => void = () => {};
		const { api, sent } = fakeApi(
			() =>
				new Promise((_, reject) => {
					refuse = reject;
				}),
		);
		const mounted = mount(() => <General />, { api });
		dispose = mounted.dispose;

		await page.getByRole("textbox", { name: "Name" }).fill("Hash Browns");
		await page.getByRole("radio", { name: "Private" }).click();
		await save().click();

		expect(sent).toEqual([
			{
				method: "PATCH",
				path: "/v0/projects/hashbrown",
				body: { name: "Hash Browns", visibility: "private" },
			},
		]);
		expect(project(mounted.client)?.name).toBe("Hash Browns");

		refuse(new ApiError(402, "client", '{"message":"No plan"}'));
		const refusal = page.getByRole("alert");
		await expect
			.element(refusal)
			.toHaveTextContent("private needs a paid plan");
		await expect
			.element(refusal.getByRole("link", { name: "See plans" }))
			.toHaveAttribute("href", "/console/organizations/pompeii-llc/billing");
		expect(project(mounted.client)).toEqual(PROJECT);
		await expect
			.element(page.getByRole("textbox", { name: "Name" }))
			.toHaveValue("Hash Browns");
		await expect
			.element(page.getByRole("radio", { name: "Private" }))
			.toBeChecked();
		await expect.element(save()).toBeEnabled();
	});

	// Kills a form left dirty after the API accepts, one that shows what the
	// reader typed rather than what the API kept, and a save that suspends the
	// page and drops the reader's focus.
	test("holds the API's answer, and the form is clean and says so", async () => {
		const { api } = fakeApi(async () => ({
			...PROJECT,
			url: "https://github.com/pompeii-llc/hash-browns/",
		}));
		const mounted = mount(() => <General />, { api });
		dispose = mounted.dispose;

		const url = page.getByRole("textbox", { name: "URL" });
		await url.fill("https://github.com/pompeii-llc/hash-browns");
		const fellBack = watchFallback(mounted.root);
		await save().click();

		await expect.element(page.getByRole("status")).toHaveTextContent("Saved");
		await expect
			.element(url)
			.toHaveValue("https://github.com/pompeii-llc/hash-browns/");
		expect(project(mounted.client)?.url).toBe(
			"https://github.com/pompeii-llc/hash-browns/",
		);
		await expect.element(save()).toBeDisabled();
		expect(
			page.getByRole("button", { name: "Discard" }).elements(),
		).toHaveLength(0);
		// A save writes the cache twice and never swaps the page for the fallback.
		expect(fellBack()).toBe(false);
	});

	// Kills a slug the API would refuse sent anyway, and Discard that keeps
	// the edit.
	test("waits for a valid slug, and Discard puts the project back in the form", async () => {
		const { api, sent } = fakeApi();
		dispose = mount(() => <General />, { api }).dispose;

		const slug = page.getByRole("textbox", { name: "Slug" });
		await slug.fill("Hash Browns");
		await expect
			.element(page.getByText("Up to 64 lowercase letters"))
			.toBeVisible();
		await expect.element(save()).toBeDisabled();
		expect(sent).toEqual([]);

		await page.getByRole("button", { name: "Discard" }).click();
		await expect.element(slug).toHaveValue("hashbrown");
		await expect.element(save()).toBeDisabled();
	});
});

// Kills a slug change that leaves the old slug's cache answering for a
// project no longer there, and one that leaves the old page for Back.
test("a slug change moves the page, forgets the old slug, and leaves it out of the history", async () => {
	const { api } = fakeApi(async () => ({ ...PROJECT, slug: "hash-browns" }));
	const mounted = mount(() => <General />, { api });
	dispose = mounted.dispose;
	await page.getByRole("textbox", { name: "Slug" }).fill("hash-browns");
	await save().click();
	await expect.poll(() => mounted.history.get()).toBe("/hash-browns/settings");
	await expect
		.poll(() =>
			mounted.client.getQueryData(["console", "project", "hashbrown"]),
		)
		.toBeUndefined();
	expect(
		mounted.client.getQueryData(["console", "project", "hash-browns"]),
	).toMatchObject({ project: { slug: "hash-browns" } });
	mounted.history.back();
	expect(mounted.history.get()).toBe("/hash-browns/settings");
	localStorage.removeItem(VERSION_KEY);
});

// Kills an answer from elsewhere that overwrites what the reader is typing,
// and one that never reaches a form the reader has not touched.
test("a change made elsewhere reaches an untouched form and spares an edited one", async () => {
	const mounted = mount(() => <General />, { api: fakeApi().api });
	dispose = mounted.dispose;
	const elsewhere = (name: string) =>
		mounted.client.setQueryData(["console", "project", "hashbrown"], {
			...bootstrapOf(),
			project: { ...PROJECT, name },
		});
	const url = page.getByRole("textbox", { name: "URL" });

	elsewhere("Hash Browns");
	await expect
		.element(page.getByRole("textbox", { name: "Name" }))
		.toHaveValue("Hash Browns");
	await url.fill("https://example.com/typing");
	elsewhere("Rosti");
	await new Promise((resolve) => setTimeout(resolve, 50));
	await expect.element(url).toHaveValue("https://example.com/typing");
});

// Kills a taken slug said only in a line no screen reader announces.
test("a slug another project has is announced under Slug", async () => {
	const { api } = fakeApi(async () => {
		throw new ApiError(409, "client", '{"message":"UNIQUE constraint failed"}');
	});
	dispose = mount(() => <General />, { api }).dispose;
	await page.getByRole("textbox", { name: "Slug" }).fill("tater-tot");
	await save().click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Another project already has this slug.");
});

describe("focus", () => {
	const answered = () =>
		fakeApi(async () => ({ ...PROJECT, name: "Hash Browns" }));
	const onBody = () =>
		document.activeElement === document.body || document.activeElement === null;

	// Kills a Save that disables itself under the reader's focus, which drops
	// a keyboard or screen reader to the top of the page.
	test("a save from the button leaves focus on the form", async () => {
		dispose = mount(() => <General />, { api: answered().api }).dispose;
		await page.getByRole("textbox", { name: "Name" }).fill("Hash Browns");
		await save().click();
		await expect.element(page.getByRole("status")).toHaveTextContent("Saved");
		expect(onBody()).toBe(false);
		await expect
			.element(page.getByRole("heading", { level: 2, name: "Project" }))
			.toHaveFocus();
	});

	// Kills a save by Enter that moves focus out of the field being edited.
	test("a save by Enter leaves focus in the field", async () => {
		dispose = mount(() => <General />, { api: answered().api }).dispose;
		const name = page.getByRole("textbox", { name: "Name" });
		await name.fill("Hash Browns");
		await userEvent.keyboard("{Enter}");
		await expect.element(page.getByRole("status")).toHaveTextContent("Saved");
		await expect.element(name).toHaveFocus();
	});

	// Kills a Discard that removes itself under the reader's focus.
	test("Discard by keyboard leaves focus on the form", async () => {
		dispose = mount(() => <General />, { api: fakeApi().api }).dispose;
		await page.getByRole("textbox", { name: "Name" }).fill("Hash Browns");
		const discard = page.getByRole("button", { name: "Discard" });
		discard.element().focus();
		await userEvent.keyboard("{Enter}");
		await expect
			.element(page.getByRole("textbox", { name: "Name" }))
			.toHaveValue("Hashbrown");
		expect(onBody()).toBe(false);
		await expect
			.element(page.getByRole("heading", { level: 2, name: "Project" }))
			.toHaveFocus();
	});
});

describe("what a reader sees", () => {
	const shown = (bootstrap: JsonConsoleProject) => {
		dispose = mount(() => <General />, {
			api: fakeApi().api,
			bootstrap,
		}).dispose;
		return {
			form: page.getByRole("textbox", { name: "Name" }).elements().length,
			danger: page.getByRole("button", { name: "Delete project" }).elements()
				.length,
			readOnly: page.getByText("Read only. Ask a project Maintainer").elements()
				.length,
		};
	};

	// Kills the Danger section drawn from `edit` or `delete` instead of `manage`.
	test("a Developer edits the project and has no Danger section", () => {
		expect(shown(bootstrapOf({ manage: false }))).toEqual({
			form: 1,
			danger: 0,
			readOnly: 0,
		});
	});

	// Kills a form drawn for a reader the API would refuse.
	test("a reader without edit sees the project as text", () => {
		const view = shown(
			bootstrapOf({ create: false, edit: false, delete: false, manage: false }),
		);
		expect(view).toEqual({ form: 0, danger: 0, readOnly: 1 });
		expect(
			page.getByText("hashbrown", { exact: true }).elements(),
		).toHaveLength(1);
	});

	// Kills a stored URL drawn as a link whatever its scheme, which runs script
	// in the console for every reader who follows it.
	test("the project's URL is a link only when it opens a web page", () => {
		const viewer = { create: false, edit: false, delete: false, manage: false };
		const script = "javascript:alert(document.domain)";
		const bootstrap = bootstrapOf(viewer);
		dispose = mount(() => <General />, {
			api: fakeApi().api,
			bootstrap: {
				...bootstrap,
				project: { ...bootstrap.project, url: script },
			},
		}).dispose;
		expect(page.getByText(script).elements()).toHaveLength(1);
		expect(page.getByRole("link", { name: script }).elements()).toHaveLength(0);
		dispose();

		dispose = mount(() => <General />, {
			api: fakeApi().api,
			bootstrap: bootstrapOf(viewer),
		}).dispose;
		const link = page.getByRole("link", { name: PROJECT.url ?? "" });
		expect(link.element().getAttribute("href")).toBe(PROJECT.url);
		expect(link.element().getAttribute("rel")).toBe(
			"noopener noreferrer nofollow",
		);
	});
});
