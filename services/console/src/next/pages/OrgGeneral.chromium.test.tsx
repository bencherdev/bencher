import { afterEach, describe, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import type { JsonConsoleUser } from "../../types/bencher";
import { ApiError } from "../api";
import { CLASSIC_ORGANIZATION } from "../memory";
import { organizationQuery } from "../organization/data";
import { ORGANIZATION, mount } from "../organization/testing";
import { organizationPath } from "../paths";
import { consoleUserQuery } from "../queries";
import { fakeApi } from "../settings/testing";
import OrgGeneral from "./OrgGeneral";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	vi.restoreAllMocks();
	localStorage.clear();
});

/** General for Pompeii LLC, the API answering each change with the organization changed. */
const open = (
	options: Partial<Parameters<typeof mount>[1]> = {},
	answer: Parameters<typeof fakeApi>[0] = async ({ body }) => ({
		...ORGANIZATION,
		...(body as object),
	}),
) => {
	const { api, sent } = fakeApi(answer);
	const mounted = mount(() => <OrgGeneral />, { api, ...options });
	dispose = mounted.dispose;
	const bootstrap = () =>
		mounted.client.getQueryData<JsonConsoleUser>(
			consoleUserQuery(api, "u").queryKey,
		);
	return { ...mounted, api, sent, bootstrap };
};

const textbox = (name: string) => page.getByRole("textbox", { name });
const button = (name: string) => page.getByRole("button", { name });

// Kills a rename sent to another organization, or with fields the reader left
// alone, a name change that asks to confirm, and a switcher left on the old name.
test("a Leader renames the organization, and the switcher follows", async () => {
	const { sent, bootstrap } = open();
	await expect.element(textbox("Name")).toHaveValue("Pompeii LLC");
	await expect.element(textbox("Slug")).toHaveValue("pompeii-llc");

	await textbox("Name").fill("Pompeii Inc");
	await button("Save changes").click();

	await expect
		.element(page.getByRole("main").getByRole("status"))
		.toHaveTextContent("Saved");
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/organizations/o1",
			body: { name: "Pompeii Inc" },
		},
	]);
	expect(bootstrap()?.organizations[0]?.name).toBe("Pompeii Inc");
	expect(document.querySelector('[role="alertdialog"]')).toBeNull();
});

// Kills a slug change sent before the reader confirms, a Cancel that sends it
// or throws the change away, a page left at the old slug, a switcher that
// keeps it, and the old slug's cache kept.
test("a slug change confirms on save, then moves the page", async () => {
	const { api, client, sent, history, bootstrap } = open();
	await textbox("Slug").fill("pompeii");
	await button("Save changes").click();

	const dialog = page.getByRole("alertdialog", {
		name: "Change the slug to pompeii?",
	});
	await expect
		.element(dialog)
		.toHaveAccessibleDescription(
			"Every existing link to this organization breaks.",
		);
	expect(sent).toEqual([]);

	await dialog.getByRole("button", { name: "Cancel" }).click();
	await expect.element(dialog).not.toBeInTheDocument();
	expect(sent).toEqual([]);
	await expect.element(textbox("Slug")).toHaveValue("pompeii");

	await button("Save changes").click();
	await dialog.getByRole("button", { name: "Change slug" }).click();

	await expect
		.poll(() => history.get())
		.toBe("/organizations/pompeii/settings");
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/organizations/o1",
			body: { slug: "pompeii" },
		},
	]);
	await expect.element(textbox("Slug")).toHaveValue("pompeii");
	await expect.element(button("Save changes")).toBeDisabled();
	expect(bootstrap()?.organizations[0]?.slug).toBe("pompeii");
	await expect
		.poll(() =>
			client.getQueryData(organizationQuery(api, "pompeii-llc").queryKey),
		)
		.toBeUndefined();
	expect(
		client.getQueryData(organizationQuery(api, "pompeii").queryKey),
	).toEqual({ ...ORGANIZATION, slug: "pompeii" });
});

// Kills a refused slug change that moves the page anyway, changes the
// switcher's or the classic console's memory before the API agreed, throws
// the reader's draft away, or does not say the slug is taken.
test("a slug the API refuses as taken keeps the page and the draft", async () => {
	const classic = JSON.stringify(ORGANIZATION);
	localStorage.setItem(CLASSIC_ORGANIZATION, classic);
	const { history, bootstrap } = open({}, async () => {
		throw new ApiError(409, "client", '{"message":"Conflict"}');
	});
	await textbox("Slug").fill("taken");
	await button("Save changes").click();
	await page
		.getByRole("alertdialog")
		.getByRole("button", { name: "Change slug" })
		.click();

	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Another organization has this slug");
	expect(history.get()).toBe("/organizations/pompeii-llc/settings");
	expect(bootstrap()?.organizations[0]?.slug).toBe("pompeii-llc");
	expect(localStorage.getItem(CLASSIC_ORGANIZATION)).toBe(classic);
	await expect.element(textbox("Slug")).toHaveValue("taken");
});

// Kills a confirmed slug change that drops the focus to the page once its
// dialog closes under a Save that waits for the API.
test("a slug change confirmed from the button leaves focus on the form", async () => {
	open({}, () => new Promise(() => {}));
	await textbox("Slug").fill("pompeii");
	await button("Save changes").click();
	await page
		.getByRole("alertdialog")
		.getByRole("button", { name: "Change slug" })
		.click();
	await expect
		.element(page.getByRole("heading", { level: 2, name: "Organization" }))
		.toHaveFocus();
});

// Kills a Danger box drawn without its delete, and a delete that skips the
// confirmation.
test("Delete organization in the Danger box opens its confirmation", async () => {
	open();
	const danger = page.getByRole("region", { name: "Danger" });
	await danger.getByRole("button", { name: "Delete organization" }).click();

	await expect
		.element(page.getByRole("alertdialog", { name: "Delete pompeii-llc?" }))
		.toBeVisible();
});

// Kills a copy of anything but the organization's UUID.
test("Copy copies the organization's UUID", async () => {
	const copy = vi
		.spyOn(navigator.clipboard, "writeText")
		.mockResolvedValue(undefined);
	open();
	await button("Copy the UUID").click();

	expect(copy).toHaveBeenCalledWith("o1");
	await expect.element(button("UUID copied")).toBeVisible();
});

describe("only what the reader can do", () => {
	const shown = () => ({
		form: document.querySelectorAll("input").length,
		save: document.querySelectorAll('button[type="submit"]').length,
		danger: document.querySelectorAll('[aria-labelledby="sec-danger"]').length,
	});

	// Kills a form or a delete drawn for a reader the API would refuse.
	test("a reader who can do neither sees the organization, read only", async () => {
		open({ allowed: { edit: false, delete: false } });
		await expect
			.element(page.getByRole("main").getByText("pompeii-llc", { exact: true }))
			.toBeVisible();
		expect(shown()).toEqual({ form: 0, save: 0, danger: 0 });
	});

	// Kills edit and delete read as one permission.
	test("edit and delete each follow their own permission", async () => {
		open({ allowed: { edit: true, delete: false } });
		await expect.element(textbox("Name")).toBeVisible();
		expect(shown()).toEqual({ form: 2, save: 1, danger: 0 });
		dispose?.();

		open({ allowed: { edit: false, delete: true } });
		await expect
			.element(page.getByRole("button", { name: "Delete organization" }))
			.toBeVisible();
		expect(shown()).toEqual({ form: 0, save: 0, danger: 1 });
	});

	// Kills controls drawn before the API has said what the reader may do.
	test("nothing is drawn until the permissions are known", async () => {
		open({ allowed: null });
		await expect
			.element(page.getByRole("heading", { level: 1, name: "General" }))
			.toBeVisible();
		expect(shown()).toEqual({ form: 0, save: 0, danger: 0 });
	});
});

// Kills a page that waits for the API to paint an organization the reader's
// bootstrap already holds, by slug or by UUID.
test("an organization the reader has seen paints at once", async () => {
	open({ organization: null });
	await expect.element(textbox("Name")).toHaveValue("Pompeii LLC");
	dispose?.();

	open({ organization: null, path: "/organizations/o1/settings" });
	await expect.element(textbox("Slug")).toHaveValue("pompeii-llc");
});

describe("the sections", () => {
	const nav = (name: string) =>
		page.getByRole("navigation", { name, exact: true });

	// Kills a rail that drops a section, marks the wrong one current, sends
	// Members anywhere but its page here, or Billing anywhere but its classic page.
	test("a wide screen has the rail, with General current", async () => {
		await page.viewport(1280, 800);
		open();
		const rail = nav("Organization settings");
		await expect.element(rail).toBeVisible();
		expect(nav("Organization settings sections").query()).toBeNull();
		await expect
			.element(rail.getByRole("link", { name: "General" }))
			.toHaveAttribute("aria-current", "page");
		await expect
			.element(rail.getByRole("link", { name: "General" }))
			.toHaveAttribute("href", organizationPath("pompeii-llc"));
		await expect
			.element(rail.getByRole("link", { name: "Members" }))
			.toHaveAttribute(
				"href",
				"/next/console/organizations/pompeii-llc/members",
			);
		await expect
			.element(rail.getByRole("link", { name: "Billing" }))
			.toHaveAttribute("href", "/console/organizations/pompeii-llc/billing");
	});

	// Kills a phone that keeps the rail, or whose sections are too small to tap.
	test("a phone has the segmented control, each section 44 px tall", async () => {
		await page.viewport(414, 896);
		open();
		const segmented = nav("Organization settings sections");
		await expect.element(segmented).toBeVisible();
		expect(nav("Organization settings").query()).toBeNull();
		for (const link of segmented.getByRole("link").elements()) {
			expect(link.getBoundingClientRect().height).toBeGreaterThanOrEqual(44);
		}
		await expect
			.element(segmented.getByRole("link", { name: "General" }))
			.toHaveAttribute("aria-current", "page");
	});
});
