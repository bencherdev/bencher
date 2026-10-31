import { afterEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { JsonConsoleUser } from "../../types/bencher";
import { ApiError } from "../api";
import { FlushContext } from "../flush";
import {
	CLASSIC_ORGANIZATION,
	VERSION_KEY,
	rememberedVersion,
} from "../memory";
import { consoleUserQuery } from "../queries";
import { fakeApi } from "../settings/testing";
import { organizationQuery } from "./data";
import DeleteOrganization from "./DeleteOrganization";
import { BOOTSTRAP, ORGANIZATION, mount } from "./testing";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	localStorage.clear();
});

const open = (
	answer?: Parameters<typeof fakeApi>[0],
	flush?: () => Promise<void>,
) => {
	const { api, sent } = fakeApi(answer);
	const left: string[] = [];
	const mounted = mount(
		() => (
			<FlushContext.Provider value={flush ?? (async () => {})}>
				<DeleteOrganization
					organization={ORGANIZATION}
					onDismiss={() => {}}
					leave={(href) => left.push(href)}
				/>
			</FlushContext.Provider>
		),
		{ api },
	);
	dispose = mounted.dispose;
	const dialog = page.getByRole("alertdialog", { name: "Delete pompeii-llc?" });
	return {
		api,
		client: mounted.client,
		sent,
		left,
		dialog,
		typed: dialog.getByRole("textbox", { name: "Type pompeii-llc to confirm" }),
		confirm: dialog.getByRole("button", { name: "Delete organization" }),
	};
};

// Kills a confirmation that names another organization's projects, or none.
test("the confirmation names the projects that go with the organization", async () => {
	const { dialog } = open();
	await expect
		.element(dialog)
		.toHaveAccessibleDescription("Gizmo and Windmill go with it.");
});

// Kills a gate that takes a prefix or another word, or compares with the name
// instead of the slug, or one that refuses the slug a touch keyboard
// capitalized or padded; a delete sent to the wrong organization; a
// reader left on a page for an organization that is gone; the organization,
// its projects, or the classic console's memory of it kept; and another
// organization's project forgotten with it.
test("Delete organization stays off until the slug is typed, then deletes and leaves", async () => {
	localStorage.setItem(
		VERSION_KEY,
		JSON.stringify({ gizmo: 1, windmill: 1, cathode: 1 }),
	);
	localStorage.setItem(CLASSIC_ORGANIZATION, JSON.stringify(ORGANIZATION));
	const { api, client, sent, left, typed, confirm } = open();
	client.setQueryData(["console", "project", "gizmo"], {});
	client.setQueryData(["console", "project", "cathode"], {});
	await expect.element(typed).toHaveFocus();
	for (const text of ["", "pompeii-l", "pompeii-llcx", "Pompeii LLC"]) {
		await typed.fill(text);
		await expect.element(confirm).toBeDisabled();
	}
	await typed.fill(" Pompeii-LLC ");
	await expect.element(confirm).toBeEnabled();

	await confirm.click();
	await expect.poll(() => left).toEqual(["/console/organizations"]);
	expect(sent).toEqual([
		{ method: "DELETE", path: "/v0/organizations/o1", body: undefined },
	]);
	expect(
		client.getQueryData<JsonConsoleUser>(consoleUserQuery(api, "u").queryKey),
	).toEqual({
		...BOOTSTRAP,
		organizations: [BOOTSTRAP.organizations[1]],
		projects: [BOOTSTRAP.projects[2]],
	});
	expect(
		client.getQueryData(organizationQuery(api, "pompeii-llc").queryKey),
	).toBeUndefined();
	expect(client.getQueryData(["console", "project", "gizmo"])).toBeUndefined();
	expect(client.getQueryData(["console", "project", "cathode"])).toEqual({});
	expect(rememberedVersion(localStorage, "gizmo")).toBeUndefined();
	expect(rememberedVersion(localStorage, "windmill")).toBeUndefined();
	expect(rememberedVersion(localStorage, "cathode")).toBe(1);
	expect(localStorage.getItem(CLASSIC_ORGANIZATION)).toBeNull();
});

// Kills a refused delete that leaves anyway, says nothing, or forgets the
// organization, its projects, or the classic console's memory of it before
// the API agreed.
test("a refused delete says why and keeps everything", async () => {
	const versions = JSON.stringify({ gizmo: 1, windmill: 1, cathode: 1 });
	localStorage.setItem(VERSION_KEY, versions);
	localStorage.setItem(CLASSIC_ORGANIZATION, JSON.stringify(ORGANIZATION));
	const { api, client, left, typed, confirm } = open(async () => {
		throw new ApiError(403, "forbidden", '{"message":"Not a Leader"}');
	});
	client.setQueryData(["console", "project", "gizmo"], {});
	await typed.fill("pompeii-llc");
	await confirm.click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not delete the organization: Not a Leader");
	expect(left).toEqual([]);
	expect(
		client.getQueryData<JsonConsoleUser>(consoleUserQuery(api, "u").queryKey),
	).toEqual(BOOTSTRAP);
	expect(
		client.getQueryData(organizationQuery(api, "pompeii-llc").queryKey),
	).toEqual(ORGANIZATION);
	expect(client.getQueryData(["console", "project", "gizmo"])).toEqual({});
	expect(localStorage.getItem(VERSION_KEY)).toBe(versions);
	expect(localStorage.getItem(CLASSIC_ORGANIZATION)).toBe(
		JSON.stringify(ORGANIZATION),
	);
});

// Kills a confirm that sends the delete again while the first is in flight.
test("a second press while the delete is in flight sends nothing more", async () => {
	let answer = () => {};
	const { sent, typed, confirm } = open(
		() =>
			new Promise((resolve) => {
				answer = () => resolve(undefined);
			}),
	);
	await typed.fill("pompeii-llc");
	await confirm.click();
	await expect.element(confirm).toBeDisabled();
	(confirm.element() as HTMLButtonElement).click();
	expect(sent).toHaveLength(1);
	answer();
});

// Kills a delete from a page reached by UUID, as the switcher links it, that
// keeps the organization cached under the UUID.
test("a page reached by UUID forgets the organization under its UUID", async () => {
	const { api, sent } = fakeApi();
	const left: string[] = [];
	const mounted = mount(
		() => (
			<FlushContext.Provider value={async () => {}}>
				<DeleteOrganization
					organization={ORGANIZATION}
					onDismiss={() => {}}
					leave={(href) => left.push(href)}
				/>
			</FlushContext.Provider>
		),
		{ api, path: "/organizations/o1/settings" },
	);
	dispose = mounted.dispose;
	expect(
		mounted.client.getQueryData(organizationQuery(api, "o1").queryKey),
	).toEqual(ORGANIZATION);
	const dialog = page.getByRole("alertdialog", { name: "Delete pompeii-llc?" });
	await dialog
		.getByRole("textbox", { name: "Type pompeii-llc to confirm" })
		.fill("pompeii-llc");
	await dialog.getByRole("button", { name: "Delete organization" }).click();

	await expect.poll(() => left).toEqual(["/console/organizations"]);
	expect(sent).toHaveLength(1);
	expect(
		mounted.client.getQueryData(organizationQuery(api, "o1").queryKey),
	).toBeUndefined();
});

// Kills a delete that leaves before the cache has written the organization's
// removal, so the browser's store keeps it.
test("Delete leaves once the cache has written the removal", async () => {
	const held: { kept: number; write: () => void }[] = [];
	let read = () => 0;
	const { api, client, left, typed, confirm } = open(
		undefined,
		() =>
			new Promise<void>((write) => {
				held.push({ kept: read(), write });
			}),
	);
	read = () =>
		client.getQueryData<JsonConsoleUser>(consoleUserQuery(api, "u").queryKey)
			?.organizations.length ?? 0;
	await typed.fill("pompeii-llc");
	await confirm.click();

	await expect.poll(() => held.length).toBe(1);
	expect(held[0]?.kept).toBe(1);
	await new Promise((resolve) => setTimeout(resolve, 100));
	expect(left).toEqual([]);
	held[0]?.write();
	await expect.poll(() => left).toEqual(["/console/organizations"]);
});
