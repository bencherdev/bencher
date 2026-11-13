import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonUser } from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { READER, mountAccount } from "../account/testing";
import { axeInBothThemes } from "../axe/testing";
import { screenBatch } from "../list/rows";
import type { Reader } from "../reader";
import Users from "./Users";

const ADMIN: Reader = { ...READER, admin: true };

const user = (name: string, slug: string, marks: Partial<JsonUser> = {}) => ({
	uuid: `uuid-${slug}`,
	name,
	slug,
	email: `${slug}@nowhere.example`,
	admin: false,
	locked: false,
	...marks,
});
const USERS: JsonUser[] = [
	user("Courage Bagge", "courage", { locked: true }),
	{
		...user("Muriel Bagge", "muriel-bagge", { admin: true }),
		uuid: READER.uuid,
	},
	user("Eustace Bagge", "eustace"),
];

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

/** The users API, filtering by the search it is asked for, and every path it read. */
const usersApi = (fail?: number) => {
	const reads: string[] = [];
	const api: Api = {
		get: async <T,>(path: string) => {
			reads.push(path);
			if (fail === 403) {
				throw new ApiError(403, "forbidden", "{}");
			}
			if (fail) {
				throw new ApiError(fail, "server", "{}");
			}
			const search =
				new URL(path, "https://bencher.example").searchParams.get("search") ??
				"";
			const found = USERS.filter((each) =>
				each.name.toLowerCase().includes(search.toLowerCase()),
			);
			return {
				data: found as T,
				headers: new Headers({ "X-Total-Count": String(found.length) }),
			};
		},
		send: async () => {
			throw new Error("The Users page changes nothing");
		},
	};
	return { api, reads };
};

const table = () => page.getByRole("table", { name: "Users" });
const names = () =>
	[...document.querySelectorAll("tbody tr .namelink")].map(
		(link) => link.textContent,
	);

// Kills a list read from anywhere but the server's users a screen at a time,
// rows that lead anywhere but each user's Profile, marks shown on the wrong
// rows, a count that is not the API's, and a rail that does not mark Users.
test("lists every user, each leading to their Profile", async () => {
	const { api, reads } = usersApi();
	dispose = mountAccount(Users, "users", { api, reader: ADMIN }).dispose;

	await expect.element(table().getByText("Courage Bagge")).toBeVisible();
	expect(reads).toEqual([`/v0/users?per_page=${screenBatch()}&page=1`]);
	expect(names()).toEqual(["Courage Bagge", "Muriel Bagge", "Eustace Bagge"]);
	await expect
		.element(table().getByRole("link", { name: "Eustace Bagge" }))
		.toHaveAttribute("href", "/console/users/eustace/settings");
	const row = (name: string) =>
		[...document.querySelectorAll("tbody tr")]
			.find((each) => each.textContent?.includes(name))
			?.textContent?.replace(/\s+/g, " ");
	expect(row("Courage Bagge")).toContain("locked");
	expect(row("Courage Bagge")).not.toContain("admin");
	expect(row("Muriel Bagge")).toContain("you admin");
	expect(row("Eustace Bagge")).not.toMatch(/you|admin|locked/);
	await expect.element(page.getByText("3 users")).toBeVisible();
	await expect
		.element(
			page
				.getByRole("navigation", { name: "Account sections" })
				.getByRole("link", { name: "Users" }),
		)
		.toHaveAttribute("aria-current", "page");
});

// Kills a search sent on every keystroke, one never sent, and one that does
// not reach the API as its search.
test("searches once the reader stops typing", async () => {
	const { api, reads } = usersApi();
	dispose = mountAccount(Users, "users", { api, reader: ADMIN }).dispose;
	await expect.element(table().getByText("Courage Bagge")).toBeVisible();

	await userEvent.type(
		page.getByRole("searchbox", { name: "Search users" }),
		"eus",
	);
	await expect.poll(() => names()).toEqual(["Eustace Bagge"]);
	expect(reads).toEqual([
		`/v0/users?per_page=${screenBatch()}&page=1`,
		`/v0/users?per_page=${screenBatch()}&page=1&search=eus`,
	]);
	await expect.element(page.getByText("1 user")).toBeVisible();
});

const usersSection = () =>
	page
		.getByRole("navigation", { name: "Account sections" })
		.getByRole("link", { name: "Users" });

// Kills a failed read shown as no users, a Retry that asks nothing, and a
// failure of the API's own taken for a refusal that drops the reader's mark.
test("a failed read says so, and Retry asks again", async () => {
	const { api, reads } = usersApi(500);
	const mounted = mountAccount(Users, "users", { api, reader: ADMIN });
	dispose = mounted.dispose;

	const alert = page.getByRole("alert");
	await expect.element(alert).toHaveTextContent("Users did not load");
	await alert.getByRole("button", { name: "Retry" }).click();
	await expect.poll(() => reads.length).toBe(2);
	expect(mounted.reader()).toEqual(ADMIN);
	await expect.element(usersSection()).toBeVisible();
});

// Kills a reader the API no longer takes for a server admin who keeps the
// mark and the Users section, which then leads only to this refusal.
test("a refused read drops the reader's admin mark and the Users section at once", async () => {
	const mounted = mountAccount(Users, "users", {
		api: usersApi(403).api,
		reader: ADMIN,
	});
	dispose = mounted.dispose;

	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Users did not load");
	await expect.poll(() => mounted.reader()).toEqual(READER);
	await expect.poll(() => usersSection().query()).toBeNull();
});

// Kills a Users page that fails axe in either theme.
test("passes axe in both themes", async () => {
	await axeInBothThemes(async () => {
		const mounted = mountAccount(Users, "users", {
			api: usersApi().api,
			reader: ADMIN,
		});
		await expect.element(table().getByText("Courage Bagge")).toBeVisible();
		return mounted.dispose;
	});
});
