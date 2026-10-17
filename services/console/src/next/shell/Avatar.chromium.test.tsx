import "../switcher/switcher.css";
import { afterEach, describe, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonConsoleUser } from "../../types/bencher";
import { fakeApi, mount } from "../settings/testing";
import Shell from "./Shell";

const READER: JsonConsoleUser = {
	uuid: "u",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	organizations: [{ uuid: "o1", name: "Pompeii LLC", slug: "pompeii-llc" }],
	projects: [
		{
			uuid: "p",
			name: "Hashbrown",
			slug: "hashbrown",
			organization: "o1",
			bmf_version: 1,
			last_report: Date.now(),
			active_alerts: 0,
		},
	],
};
const USER = {
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	email: "muriel@nowhere.example",
};

let unmount: (() => void) | undefined;
afterEach(() => {
	unmount?.();
	document.body.replaceChildren();
	localStorage.clear();
});

/**
 * The shell on Hashbrown's Runs, or on an account page, at this width; with
 * `asked`, nothing is cached and every read is recorded there, unanswered.
 */
const open = async (
	width: number,
	{
		project = true,
		signOut = () => {},
		asked,
	}: { project?: boolean; signOut?: () => void; asked?: string[] } = {},
) => {
	await page.viewport(width, 844);
	unmount = mount(
		() =>
			project ? (
				<Shell
					slug="hashbrown"
					tab="runs"
					organization={{ name: "Pompeii LLC", uuid: "o1" }}
					project="Hashbrown"
					reader="u"
					user={USER}
					signOut={signOut}
				/>
			) : (
				<Shell user={USER} signOut={signOut} />
			),
		{
			api: fakeApi(undefined, (path) => {
				asked?.push(path);
				return new Promise(() => {});
			}).api,
			path: "/hashbrown/runs",
			cached: asked ? [] : [[["console", "user", "u"], READER]],
		},
	).dispose;
};

const avatar = () => page.getByRole("button", { name: "Account" });
const menu = () => page.getByRole("menu", { name: "Account" });
const item = (name: string | RegExp) => page.getByRole("menuitem", { name });
const items = () =>
	[...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].map(
		(entry) => entry.textContent,
	);

describe("wide", () => {
	// Kills an entry dropped or led to the wrong place, Profile left outside the
	// new console or Keys and Organizations moved into it before their pages,
	// and Switch project offered where the crumb already is.
	test("the avatar's menu leads to Profile, Keys, Organizations, and Help", async () => {
		await open(1280);
		await avatar().click();

		await expect.element(menu()).toBeVisible();
		expect(items()).toEqual([
			"Profile",
			"Keys",
			"Organizations",
			"Help",
			"Sign out",
		]);
		const href = (name: string) =>
			(item(name).element() as HTMLAnchorElement).getAttribute("href");
		expect(href("Profile")).toBe("/next/console/users/muriel-bagge/settings");
		expect(href("Keys")).toBe("/console/users/muriel-bagge/keys");
		expect(href("Organizations")).toBe("/console/organizations");
		expect(href("Help")).toBe("/help/");
	});

	// Kills a menu the keyboard cannot open with Enter or Space, arrows that do
	// not move through it, an Escape that leaves it open or drops the focus, and
	// a button that does not report it open.
	test("Enter and Space open it on its first entry, the arrows move, and Escape gives the focus back", async () => {
		await open(1280);

		for (const key of ["{Enter}", " "]) {
			(avatar().element() as HTMLElement).focus();
			await userEvent.keyboard(key);
			await expect.element(item("Profile")).toHaveFocus();
			await expect.element(avatar()).toHaveAttribute("aria-expanded", "true");
			await userEvent.keyboard("{ArrowDown}");
			await expect.element(item("Keys")).toHaveFocus();
			await userEvent.keyboard("{ArrowUp}{ArrowUp}");
			await expect.element(item("Sign out")).toHaveFocus();

			await userEvent.keyboard("{Escape}");
			await expect.element(menu()).not.toBeInTheDocument();
			await expect.element(avatar()).toHaveFocus();
			await expect.element(avatar()).toHaveAttribute("aria-expanded", "false");
		}
	});

	// Kills a Sign out that does not end the session, ends it twice, or sends
	// the next reader back to this one's page.
	test("Sign out signs the reader out", async () => {
		const signOut = vi.fn();
		await open(1280, { signOut });
		await avatar().click();

		await item("Sign out").click();

		expect(signOut.mock.calls).toEqual([[false]]);
	});

	// Kills the reader's projects asked for where the menu offers no switch.
	test("opening the menu asks for nothing", async () => {
		const asked: string[] = [];
		await open(1280, { asked });
		await avatar().click();
		await expect.element(menu()).toBeVisible();

		expect(asked).toEqual([]);
	});

	// Kills a press on the avatar that cannot close what it opened.
	test("a second press on the avatar closes the menu", async () => {
		await open(1280);
		await avatar().click();
		await expect.element(menu()).toBeVisible();

		await avatar().click();

		await expect.element(menu()).not.toBeInTheDocument();
	});
});

describe("phone", () => {
	// Kills a phone menu with no way to switch projects, a switcher that opens
	// as the crumb's popover (hidden on a phone) rather than a modal sheet, a
	// sheet that does not start on its search, and a close that drops the focus.
	test("Switch project leads the menu and opens the switcher as a sheet", async () => {
		await open(390);
		await avatar().click();
		expect(items()[0]).toContain("Switch project");

		await item(/^Switch project/).click();

		const sheet = page.getByRole("dialog", { name: "Switch project" });
		await expect.element(sheet).toBeVisible();
		expect((sheet.element() as HTMLDialogElement).matches(":modal")).toBe(true);
		await expect
			.element(page.getByRole("combobox", { name: "Search projects" }))
			.toHaveFocus();
		await expect
			.element(page.getByRole("option", { name: /^Hashbrown/ }))
			.toBeVisible();
		await expect.element(menu()).not.toBeInTheDocument();

		await userEvent.keyboard("{Escape}");
		await expect.element(sheet).not.toBeInTheDocument();
		await expect.element(avatar()).toHaveFocus();
	});

	// Kills the sheet's Close button leaving it open or dropping the focus.
	test("the sheet's Close button closes it and gives the focus back", async () => {
		await open(390);
		await avatar().click();
		await item(/^Switch project/).click();

		await page.getByRole("button", { name: "Close" }).click();

		await expect
			.element(page.getByRole("dialog", { name: "Switch project" }))
			.not.toBeInTheDocument();
		await expect.element(avatar()).toHaveFocus();
	});

	// Kills entries a thumb cannot hit, and a menu drawn off the screen.
	test("every entry is a 44 px target on the screen", async () => {
		await open(390);
		await avatar().click();
		await expect.element(menu()).toBeVisible();

		const entries = document.querySelectorAll('[role="menuitem"]');
		expect(entries).toHaveLength(6);
		for (const entry of entries) {
			const box = entry.getBoundingClientRect();
			expect(box.height, entry.textContent ?? "").toBeGreaterThanOrEqual(44);
			expect(box.left).toBeGreaterThanOrEqual(0);
			expect(box.right).toBeLessThanOrEqual(window.innerWidth);
		}
	});

	// Kills a phone menu that waits for Switch project to start the reader's
	// projects, which the sheet then opens without.
	test("opening the menu asks for the reader's projects", async () => {
		const asked: string[] = [];
		await open(390, { asked });
		await avatar().click();
		await expect.element(menu()).toBeVisible();

		expect(asked).toEqual(["/v0/console/users/u"]);
	});

	// Kills a Profile entry that leaves its menu open when the page under it
	// stays, as Profile's own page does.
	test("Profile closes the menu", async () => {
		await open(390, { project: false });
		await avatar().click();

		await item("Profile").click();

		await expect.element(menu()).not.toBeInTheDocument();
	});

	// Kills Switch project offered on an account page, which has no project to
	// switch from.
	test("an account page offers no Switch project", async () => {
		await open(390, { project: false });
		await avatar().click();

		await expect.element(menu()).toBeVisible();
		expect(items()[0]).toBe("Profile");
	});
});
