import { type JsonConsoleProject, Visibility } from "../../types/bencher";
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
	// that keeps the refused change, a refusal said nowhere, and a form that
	// drops the reader's edits when it is refused.
	test("shows at once, and a refusal puts the project back and says why", async () => {
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
		await save().click();

		expect(sent).toEqual([
			{
				method: "PATCH",
				path: "/v0/projects/hashbrown",
				body: { name: "Hash Browns" },
			},
		]);
		expect(project(mounted.client)?.name).toBe("Hash Browns");

		refuse(new ApiError(400, "client", '{"message":"Bad name"}'));
		await expect.element(page.getByRole("alert")).toHaveTextContent("Bad name");
		expect(project(mounted.client)).toEqual(PROJECT);
		await expect
			.element(page.getByRole("textbox", { name: "Name" }))
			.toHaveValue("Hash Browns");
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
			.element(page.getByText("Lowercase letters, numbers, and single dashes"))
			.toBeVisible();
		await expect.element(save()).toBeDisabled();
		expect(sent).toEqual([]);

		await page.getByRole("button", { name: "Discard" }).click();
		await expect.element(slug).toHaveValue("hashbrown");
		await expect.element(save()).toBeDisabled();
	});
});

/** The confirmation a slug change opens on save. */
const slugDialog = (slug: string) =>
	page.getByRole("alertdialog", { name: `Change the slug to ${slug}?` });

// Kills a slug change sent before the reader confirms, a Cancel that sends it
// or throws the draft away, a Change slug that sends nothing, and a warning
// line left under the field, and the confirmation's code loaded with the page.
test("a slug change confirms on save, and the field carries no warning", async () => {
	const { api, sent } = fakeApi(async () => ({
		...PROJECT,
		slug: "hash-browns",
	}));
	dispose = mount(() => <General />, { api }).dispose;
	const slug = page.getByRole("textbox", { name: "Slug" });
	await slug.fill("hash-browns");
	await expect.element(slug).toHaveAccessibleDescription("");
	expect(slug.element().closest(".formrow")?.textContent).toBe("Slug");
	expect(
		performance
			.getEntriesByType("resource")
			.some(({ name }) => name.includes("SlugDialog")),
	).toBe(false);
	await save().click();

	const dialog = slugDialog("hash-browns");
	await expect
		.element(dialog)
		.toHaveAccessibleDescription("Every existing link to this project breaks.");
	expect(sent).toEqual([]);

	await dialog.getByRole("button", { name: "Cancel" }).click();
	await expect.element(dialog).not.toBeInTheDocument();
	expect(sent).toEqual([]);
	await expect.element(slug).toHaveValue("hash-browns");

	await save().click();
	await dialog.getByRole("button", { name: "Change slug" }).click();
	await expect.element(dialog).not.toBeInTheDocument();
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/projects/hashbrown",
			body: { slug: "hash-browns" },
		},
	]);
	localStorage.removeItem(VERSION_KEY);
});

// Kills a slug change that leaves the old slug's cache answering for a
// project no longer there, and one that leaves the old page for Back.
test("a slug change moves the page, forgets the old slug, and leaves it out of the history", async () => {
	const { api } = fakeApi(async () => ({ ...PROJECT, slug: "hash-browns" }));
	const mounted = mount(() => <General />, { api });
	dispose = mounted.dispose;
	await page.getByRole("textbox", { name: "Slug" }).fill("hash-browns");
	await save().click();
	await slugDialog("hash-browns")
		.getByRole("button", { name: "Change slug" })
		.click();
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
	await slugDialog("tater-tot")
		.getByRole("button", { name: "Change slug" })
		.click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Another project has this slug");
});

describe("visibility", () => {
	const opener = () => page.getByRole("button", { name: "Change visibility" });
	/** The row's line, read from the page while the dialog holds the focus. */
	const line = () => document.getElementById("dz-vis")?.textContent;

	// Kills a change sent before the reader confirms, a Cancel that sends it,
	// and a dialog that offers the visibility the project already has.
	test("asks first, for the other visibility, and Cancel sends nothing", async () => {
		for (const [visibility, other] of [
			[Visibility.Public, Visibility.Private],
			[Visibility.Private, Visibility.Public],
		] as const) {
			const { api, sent } = fakeApi();
			dispose = mount(() => <General />, {
				api,
				bootstrap: { ...bootstrapOf(), project: { ...PROJECT, visibility } },
			}).dispose;

			await opener().click();
			const dialog = page.getByRole("alertdialog", {
				name: `Make hashbrown ${other}?`,
			});
			await expect.element(dialog).toBeVisible();
			expect(sent).toEqual([]);
			await dialog.getByRole("button", { name: "Cancel" }).click();
			await expect.element(dialog).not.toBeInTheDocument();
			expect(sent).toEqual([]);
			dispose();
		}
		dispose = undefined;
	});

	// Kills a change that waits for the API before the row says it, one that
	// sends the form's fields along, a dialog that renames itself once the
	// change shows, a dialog left open once it is made, and a focus dropped to
	// the page when the dialog goes.
	test("sends the visibility alone, shows it at once, and gives the focus back", async () => {
		let answer: (project: unknown) => void = () => {};
		const { api, sent } = fakeApi(
			() =>
				new Promise((resolve) => {
					answer = resolve;
				}),
		);
		const mounted = mount(() => <General />, { api });
		dispose = mounted.dispose;
		await page.getByRole("textbox", { name: "Name" }).fill("Hash Browns");

		await opener().click();
		await page.getByRole("button", { name: "Make private" }).click();
		expect(sent).toEqual([
			{
				method: "PATCH",
				path: "/v0/projects/hashbrown",
				body: { visibility: Visibility.Private },
			},
		]);
		expect(project(mounted.client)?.visibility).toBe(Visibility.Private);
		expect(line()).toContain("private");
		await expect
			.element(
				page.getByRole("alertdialog", { name: "Make hashbrown private?" }),
			)
			.toBeVisible();

		answer({ ...PROJECT, visibility: Visibility.Private });
		await expect.element(page.getByRole("alertdialog")).not.toBeInTheDocument();
		await expect.element(opener()).toHaveFocus();
		await expect.element(opener()).toHaveAccessibleDescription(/private/);
		await expect
			.element(page.getByRole("textbox", { name: "Name" }))
			.toHaveValue("Hash Browns");
	});

	// Kills a plan refusal said outside the dialog or without its plans link,
	// a refused change left showing, a dialog closed on a refusal, and a
	// confirm that disables itself while it waits, which drops the focus.
	test("a missing plan is said in the dialog, with the plans link, and the project stays public", async () => {
		let refuse: (error: unknown) => void = () => {};
		const { api } = fakeApi(
			() =>
				new Promise((_, reject) => {
					refuse = reject;
				}),
		);
		const mounted = mount(() => <General />, { api });
		dispose = mounted.dispose;

		await opener().click();
		const dialog = page.getByRole("alertdialog", {
			name: "Make hashbrown private?",
		});
		const confirm = dialog.getByRole("button", { name: "Make private" });
		await confirm.click();
		await new Promise((resolve) => requestAnimationFrame(resolve));
		refuse(new ApiError(402, "client", '{"message":"No plan"}'));
		const refusal = dialog.getByRole("alert");
		await expect
			.element(refusal)
			.toHaveTextContent("private needs a paid plan");
		await expect
			.element(refusal.getByRole("link", { name: "See plans" }))
			.toHaveAttribute(
				"href",
				"/next/console/organizations/pompeii-llc/billing",
			);
		expect(project(mounted.client)).toEqual(PROJECT);
		expect(line()).toContain("public");
		await expect.element(confirm).toHaveFocus();
	});

	// Kills any other refusal shown with the plans link, as if a plan would help.
	test("another refusal says what the API said, without the plans link", async () => {
		const { api } = fakeApi(async () => {
			throw new ApiError(400, "client", '{"message":"Bad visibility"}');
		});
		dispose = mount(() => <General />, { api }).dispose;

		await opener().click();
		await page.getByRole("button", { name: "Make private" }).click();
		const refusal = page.getByRole("alertdialog").getByRole("alert");
		await expect.element(refusal).toHaveTextContent("Bad visibility");
		expect(refusal.getByRole("link").elements()).toHaveLength(0);
	});
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

	// Kills a confirmed slug change that drops the focus to the page once its
	// dialog closes under a Save that waits for the API.
	test("a slug change confirmed from the button leaves focus on the form", async () => {
		const { api } = fakeApi(() => new Promise(() => {}));
		dispose = mount(() => <General />, { api }).dispose;
		await page.getByRole("textbox", { name: "Slug" }).fill("hash-browns");
		await save().click();
		await slugDialog("hash-browns")
			.getByRole("button", { name: "Change slug" })
			.click();
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
		const danger = page.getByRole("region", { name: "Danger" });
		return {
			form: page.getByRole("textbox", { name: "Name" }).elements().length,
			// The section's buttons in order, or null without the section.
			danger:
				danger.elements().length === 0
					? null
					: danger
							.getByRole("button")
							.elements()
							.map((button) => button.textContent),
			readOnly: page.getByText("Read only", { exact: true }).elements().length,
		};
	};

	// Kills a Danger section that drops a row, puts delete first, or draws the
	// visibility control anywhere else, such as back in the form.
	test("a Maintainer edits the project, and changes its visibility or deletes it under Danger", () => {
		expect(shown(bootstrapOf())).toEqual({
			form: 1,
			danger: ["Change visibility", "Delete project"],
			readOnly: 0,
		});
		expect(
			page.getByRole("button", { name: "Change visibility" }).elements(),
		).toHaveLength(1);
		expect(page.getByRole("radio").elements()).toHaveLength(0);
	});

	// Kills the delete row drawn from `edit` or `delete` instead of `manage`,
	// and the visibility row, or the whole section, kept from a reader the API
	// lets change visibility.
	test("a Developer edits the project and changes its visibility under Danger, with no delete", () => {
		expect(shown(bootstrapOf({ manage: false }))).toEqual({
			form: 1,
			danger: ["Change visibility"],
			readOnly: 0,
		});
	});

	// Kills a form drawn for a reader the API would refuse, and a Danger
	// section drawn, empty, for one who has neither of its rows.
	test("a reader without edit sees the project as text", () => {
		const view = shown(
			bootstrapOf({ create: false, edit: false, delete: false, manage: false }),
		);
		expect(view).toEqual({ form: 0, danger: null, readOnly: 1 });
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
