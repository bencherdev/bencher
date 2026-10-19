import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type {
	JsonAuthUser,
	JsonConsoleUser,
	JsonConsoleUserProject,
} from "../../types/bencher";
import { AccountContext } from "../account/context";
import { ApiError } from "../api";
import { FlushContext } from "../flush";
import { VERSION_KEY, rememberedVersion } from "../memory";
import { ORGANIZATIONS_PATH, projectPath } from "../paths";
import { USER_KEY } from "../reader";
import DeleteProject from "./DeleteProject";
import { PROJECT, fakeApi, mount } from "./testing";

const READER = {
	token: "tok",
	user: { uuid: "u" },
	expiration: "2999-01-01T00:00:00Z",
} as JsonAuthUser;

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	localStorage.clear();
});

const open = (answer?: Parameters<typeof fakeApi>[0]) => {
	const { api, sent } = fakeApi(answer);
	const left: string[] = [];
	const mounted = mount(
		() => (
			<DeleteProject
				project={PROJECT}
				onDismiss={() => {}}
				leave={(href) => left.push(href)}
			/>
		),
		{ api },
	);
	dispose = mounted.dispose;
	const dialog = page.getByRole("alertdialog", { name: "Delete hashbrown?" });
	return {
		client: mounted.client,
		sent,
		left,
		typed: dialog.getByRole("textbox", { name: "Type hashbrown to confirm" }),
		confirm: dialog.getByRole("button", { name: "Delete project" }),
	};
};

// Kills a gate that takes a prefix, ignores case or spaces, or compares with
// the name instead of the slug; a delete sent to the wrong project; a reader
// left on a page for a project that is gone; and its cache and memories kept.
test("Delete project stays off until the slug is typed exactly, then deletes and leaves", async () => {
	localStorage.setItem(VERSION_KEY, JSON.stringify({ hashbrown: 1 }));
	const { client, sent, left, typed, confirm } = open();
	await expect.element(typed).toHaveFocus();
	for (const text of [
		"",
		"hashbro",
		"Hashbrown",
		"hashbrown ",
		" hashbrown",
		"hashbrownx",
	]) {
		await typed.fill(text);
		await expect.element(confirm).toBeDisabled();
	}
	await typed.fill("hashbrown");
	await expect.element(confirm).toBeEnabled();

	await confirm.click();
	await expect.poll(() => left).toEqual([ORGANIZATIONS_PATH]);
	expect(sent).toEqual([
		{ method: "DELETE", path: "/v0/projects/hashbrown", body: undefined },
	]);
	// What the reader had of the project goes with it.
	expect(
		client.getQueryData(["console", "project", "hashbrown"]),
	).toBeUndefined();
	expect(rememberedVersion(localStorage, "hashbrown")).toBeUndefined();
});

// Kills a refused delete that leaves anyway or says nothing.
test("a refused delete says why and stays", async () => {
	const { left, typed, confirm } = open(async () => {
		throw new ApiError(403, "forbidden", '{"message":"Not a Maintainer"}');
	});
	await typed.fill("hashbrown");
	await confirm.click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not delete the project: Not a Maintainer");
	expect(left).toEqual([]);
});

// Kills a delete that leaves before the cache has written the project's
// removal, so the browser's store keeps it, and a write made before the
// removal.
test("Delete leaves once the cache has written the project's removal", async () => {
	const { api } = fakeApi();
	const left: string[] = [];
	const held: { kept: number; write: () => void }[] = [];
	const mounted = mount(
		() => (
			<FlushContext.Provider
				value={() =>
					new Promise<void>((write) => {
						const kept = mounted.client.getQueryCache().findAll({
							queryKey: ["console", "project", "hashbrown"],
						}).length;
						held.push({ kept, write });
					})
				}
			>
				<DeleteProject
					project={PROJECT}
					onDismiss={() => {}}
					leave={(href) => left.push(href)}
				/>
			</FlushContext.Provider>
		),
		{ api },
	);
	dispose = mounted.dispose;
	const dialog = page.getByRole("alertdialog", { name: "Delete hashbrown?" });
	await dialog
		.getByRole("textbox", { name: "Type hashbrown to confirm" })
		.fill("hashbrown");
	await dialog.getByRole("button", { name: "Delete project" }).click();

	await expect.poll(() => held.length).toBe(1);
	expect(held[0]?.kept).toBe(0);
	await new Promise((resolve) => setTimeout(resolve, 100));
	expect(left).toEqual([]);
	held[0]?.write();
	await expect.poll(() => left).toEqual([ORGANIZATIONS_PATH]);
});

const deleteWith = async (cached?: JsonConsoleUser) => {
	const { api } = fakeApi();
	const left: string[] = [];
	const mounted = mount(
		() => (
			<AccountContext.Provider
				value={{
					api,
					reader: () => READER,
					keepUser: () => {},
					signOut: () => {},
				}}
			>
				<DeleteProject
					project={PROJECT}
					onDismiss={() => {}}
					leave={(href) => left.push(href)}
				/>
			</AccountContext.Provider>
		),
		{ api, cached: cached ? [[["console", "user", "u"], cached]] : [] },
	);
	dispose = mounted.dispose;
	const dialog = page.getByRole("alertdialog", { name: "Delete hashbrown?" });
	await dialog
		.getByRole("textbox", { name: "Type hashbrown to confirm" })
		.fill("hashbrown");
	await dialog.getByRole("button", { name: "Delete project" }).click();
	return { left, client: mounted.client };
};

const listed = (slug: string, last_report: number) =>
	({
		uuid: slug,
		name: slug,
		slug,
		organization: "org-uuid",
		bmf_version: 1,
		last_report,
		active_alerts: 0,
	}) as JsonConsoleUserProject;

// Kills a delete that lands anywhere but the main project the browser remembers.
test("Delete lands on the main project the browser remembers", async () => {
	localStorage.setItem(USER_KEY, JSON.stringify(READER));
	localStorage.setItem(VERSION_KEY, JSON.stringify({ gizmo: 1, hashbrown: 1 }));
	const { left } = await deleteWith();
	await expect.poll(() => left).toEqual([projectPath("gizmo")]);
});

// Kills a delete that lands on the project it deleted, which the reader's
// cached projects still listed, or keeps it listed for the switcher.
test("Delete with nothing remembered lands on the newest project left", async () => {
	const cached = {
		uuid: "u",
		organizations: [],
		projects: [listed("hashbrown", 2), listed("gizmo", 1)],
	} as unknown as JsonConsoleUser;
	const { left, client } = await deleteWith(cached);
	await expect.poll(() => left).toEqual([projectPath("gizmo")]);
	expect(
		client
			.getQueryData<JsonConsoleUser>(["console", "user", "u"])
			?.projects.map(({ slug }) => slug),
	).toEqual(["gizmo"]);
});
