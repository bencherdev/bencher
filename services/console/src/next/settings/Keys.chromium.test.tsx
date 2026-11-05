import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonProjectKey } from "../../types/bencher";
import { ApiError } from "../api";
import { axeInBothThemes } from "../axe/testing";
import { KeyTable } from "./KeyList";
import Keys from "./Keys";
import { bootstrapOf, fakeApi, mount, watchFallback } from "./testing";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const key: JsonProjectKey = {
	uuid: "key-uuid",
	project: "p",
	name: "everett-laptop",
	creation: "2026-09-02T10:00:00Z",
	expiration: "2026-10-02T10:00:00Z",
};

const activeKeys: [unknown[], unknown] = [
	["console", "keys", "hashbrown", "active"],
	{ keys: [key], total: 1 },
];
const cached: [unknown[], unknown][] = [
	activeKeys,
	[["console", "keys", "hashbrown", "revoked"], { keys: [], total: 0 }],
];

// Kills a refused revoke that leaves the key shown as revoked, or says nothing.
test("a refused revoke puts the key back and says why", async () => {
	let refuse: (error: unknown) => void = () => {};
	const { api, sent } = fakeApi(
		() =>
			new Promise((_, reject) => {
				refuse = reject;
			}),
	);
	dispose = mount(() => <Keys />, { api, cached }).dispose;
	const active = page.getByRole("table", { name: "Active keys" });

	await page.getByRole("button", { name: "Revoke everett-laptop" }).click();
	await page
		.getByRole("alertdialog", { name: "Revoke everett-laptop?" })
		.getByRole("button", { name: "Revoke key" })
		.click();
	expect(sent).toEqual([
		{
			method: "DELETE",
			path: "/v0/projects/hashbrown/keys/key-uuid",
			body: undefined,
		},
	]);
	await expect
		.element(active.getByText("everett-laptop"))
		.not.toBeInTheDocument();
	await expect
		.element(page.getByRole("radio", { name: "Revoked 1" }))
		.toBeInTheDocument();

	refuse(new ApiError(503, "server", '{"message":"Busy"}'));
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Not revoked: Busy");
	await expect.element(active.getByText("everett-laptop")).toBeVisible();
	await expect
		.element(page.getByRole("radio", { name: "Active 1" }))
		.toBeInTheDocument();
});

// Kills an API reason that grows the toolbar a line at a time, and one cut
// with no way to read the rest.
test("a long refusal holds one line and opens whole in place", async () => {
	await page.viewport(390, 844);
	const reason = `Key ${"k".repeat(64)} is held by ${"a".repeat(200)}`;
	const { api } = fakeApi(async () => {
		throw new ApiError(409, "client", JSON.stringify({ message: reason }));
	});
	dispose = mount(() => <Keys />, { api, cached }).dispose;
	await page.getByRole("button", { name: "Revoke everett-laptop" }).click();
	await page
		.getByRole("alertdialog", { name: "Revoke everett-laptop?" })
		.getByRole("button", { name: "Revoke key" })
		.click();

	const alert = page.getByRole("alert");
	await expect.element(alert).toHaveTextContent(`Not revoked: ${reason}`);
	const line = Number.parseFloat(getComputedStyle(alert.element()).lineHeight);
	const height = () => alert.element().getBoundingClientRect().height;
	expect(height()).toBeLessThan(line * 1.5);
	expect(alert.element().getBoundingClientRect().right).toBeLessThanOrEqual(
		window.innerWidth,
	);
	await alert.getByRole("button").click();
	await expect
		.element(alert.getByRole("button"))
		.toHaveAttribute("aria-expanded", "true");
	expect(height()).toBeGreaterThan(line * 2);
});

// Kills a long key name that runs out of its cell, wraps down it, or pushes
// the New tag away from it.
test.each([320, 1024])(
	"at %i px a long key name holds one line in its cell",
	async (width) => {
		await page.viewport(width, 844);
		dispose = mount(
			() => (
				<KeyTable
					status="active"
					label="Active keys"
					empty="No keys"
					keys={[{ ...key, name: "k".repeat(64) }]}
					made={new Set([key.uuid])}
					onRevoke={() => {}}
				/>
			),
			{ api: fakeApi().api },
		).dispose;
		await expect
			.element(page.getByRole("table", { name: "Active keys" }))
			.toBeVisible();

		const name = document.querySelector("td b") as HTMLElement;
		expect(name.querySelector(".ui-trunc")).not.toBeNull();
		expect(name.scrollWidth).toBeLessThanOrEqual(name.clientWidth);
		expect(name.getBoundingClientRect().height).toBeLessThan(
			Number.parseFloat(getComputedStyle(name).lineHeight) * 1.5,
		);
		const tag = document.querySelector(".newtag") as HTMLElement;
		expect(
			tag.getBoundingClientRect().left - name.getBoundingClientRect().right,
		).toBeLessThan(12);
	},
);

// Kills key controls drawn for a reader the API would refuse.
test("a reader without manage sees no key and no control", () => {
	const { api } = fakeApi();
	dispose = mount(() => <Keys />, {
		api,
		cached,
		bootstrap: bootstrapOf({ manage: false }),
	}).dispose;
	expect(page.getByRole("button", { name: "New key" }).elements()).toHaveLength(
		0,
	);
	expect(page.getByRole("table").elements()).toHaveLength(0);
	expect(page.getByText("Read only", { exact: true }).elements()).toHaveLength(
		1,
	);
});

// Kills key lists that wait forever on a failed read, with no way to retry.
test("lists that did not load say so, with Retry", async () => {
	const { api } = fakeApi(undefined, async () => {
		throw new ApiError(undefined, "network", "Failed to fetch");
	});
	dispose = mount(() => <Keys />, { api }).dispose;
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("The keys did not load");
	await expect
		.element(page.getByRole("button", { name: "Retry" }))
		.toBeVisible();
});

// Kills a Revoked list that, still loading when a key is revoked, never
// learns of it, and a read of it that suspends the page and drops the focus.
test("a revoke reads the revoked list again when it had not arrived", async () => {
	const reads: string[] = [];
	const { api } = fakeApi(undefined, (path) => {
		reads.push(path);
		return new Promise(() => {});
	});
	dispose = mount(() => <Keys />, { api, cached: [activeKeys] }).dispose;
	const revokedReads = () =>
		reads.filter((path) => path.includes("revoked=true")).length;
	await expect.poll(revokedReads).toBe(1);

	await page.getByRole("button", { name: "Revoke everett-laptop" }).click();
	await page
		.getByRole("alertdialog", { name: "Revoke everett-laptop?" })
		.getByRole("button", { name: "Revoke key" })
		.click();
	await expect.poll(revokedReads).toBe(2);
	// The list reading again keeps the page, and the focus on it.
	await expect
		.element(page.getByRole("radio", { name: /^Active/ }))
		.toHaveFocus();
	expect(page.getByText("Suspended").elements()).toHaveLength(0);
});

// Kills focus left on the Revoke button the revoke removes, and a cache write
// that suspends the page, either of which drops a keyboard reader to the top.
test("after a revoke, focus is on the Status choice", async () => {
	const { api } = fakeApi();
	const mounted = mount(() => <Keys />, { api, cached });
	dispose = mounted.dispose;
	await page.getByRole("button", { name: "Revoke everett-laptop" }).click();
	const fellBack = watchFallback(mounted.root);
	await page
		.getByRole("alertdialog", { name: "Revoke everett-laptop?" })
		.getByRole("button", { name: "Revoke key" })
		.click();
	await expect
		.element(page.getByRole("radio", { name: /^Active/ }))
		.toHaveFocus();
	// A write to the cache never swaps the page for the Suspense fallback.
	expect(fellBack()).toBe(false);
});

// Kills key reads sent for a reader the API refuses them to.
test("a reader without manage never asks for keys", async () => {
	const reads: string[] = [];
	const { api } = fakeApi(undefined, (path) => {
		reads.push(path);
		return new Promise(() => {});
	});
	dispose = mount(() => <Keys />, {
		api,
		bootstrap: bootstrapOf({ manage: false }),
	}).dispose;
	await new Promise((resolve) => setTimeout(resolve, 50));
	expect(reads).toEqual([]);
});

// Kills a new key made while Revoked is showing, which then stays out of view,
// a reveal a stray Escape closes, and an export line not grouped under its label.
test("a key made from the Revoked list shows among the active ones", async () => {
	const { api } = fakeApi(async ({ method }) =>
		method === "POST"
			? {
					uuid: "new-uuid",
					project: "p",
					name: "release-benchmarks",
					key: "bencher_run_secret",
					creation: "2026-09-13T00:00:00Z",
					expiration: "2026-12-12T00:00:00Z",
				}
			: undefined,
	);
	dispose = mount(() => <Keys />, { api, cached }).dispose;
	await page.getByRole("radio", { name: /^Revoked/ }).click();
	await page.getByRole("button", { name: "New key" }).click();
	await page.getByRole("textbox", { name: "Name" }).fill("release-benchmarks");
	await page.getByRole("button", { name: "Create key" }).click();
	const reveal = page.getByRole("dialog", { name: "Copy your new key" });
	await expect
		.element(reveal.getByRole("group", { name: "Export line" }))
		.toHaveTextContent("export BENCHER_API_KEY=bencher_run_secret");
	// The one showing of the secret survives a stray Escape.
	await userEvent.keyboard("{Escape}");
	await expect.element(reveal).toBeVisible();
	await reveal.getByRole("button", { name: "Done" }).click();
	await expect
		.element(page.getByRole("radio", { name: /^Active/ }))
		.toBeChecked();
	await expect
		.element(
			page
				.getByRole("table", { name: "Active keys" })
				.getByText("release-benchmarks"),
		)
		.toBeVisible();
});

// Kills the API's name order, which buries the newest key.
test("keys are listed newest first", () => {
	const older = {
		...key,
		uuid: "older",
		name: "a-older",
		creation: "2026-06-17T00:00:00Z",
	};
	const newer = {
		...key,
		uuid: "newer",
		name: "z-newer",
		creation: "2026-09-12T00:00:00Z",
	};
	const mounted = mount(() => <Keys />, {
		api: fakeApi().api,
		cached: [
			[
				["console", "keys", "hashbrown", "active"],
				{ keys: [older, newer], total: 2 },
			],
		],
	});
	dispose = mounted.dispose;
	const rows = [...mounted.root.querySelectorAll("tbody tr")].map(
		(row) => row.textContent ?? "",
	);
	expect(rows[0]).toContain("z-newer");
	expect(rows[1]).toContain("a-older");
});

// Kills a name limit in letters, which lets through a name the API refuses
// for its bytes.
test("a key name is held to the API's 64 bytes", async () => {
	const { api, sent } = fakeApi();
	dispose = mount(() => <Keys />, { api, cached }).dispose;
	await page.getByRole("button", { name: "New key" }).click();
	await page.getByRole("textbox", { name: "Name" }).fill("é".repeat(33));
	await expect.element(page.getByText("66 of 64 bytes")).toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "Create key" }))
		.toBeDisabled();
	await page.getByRole("textbox", { name: "Name" }).fill("é".repeat(32));
	await expect
		.element(page.getByRole("button", { name: "Create key" }))
		.toBeEnabled();
	expect(sent).toEqual([]);
});

// Kills a folded row that says "Never" with nothing to say what never comes.
test("on a narrow screen, a key with no end reads Never expires", async () => {
	await page.viewport(390, 844);
	const creation = "2026-09-02T10:00:00Z";
	const never = {
		...key,
		creation,
		expiration: new Date(
			Date.parse(creation) + (2 ** 32 - 1) * 1000,
		).toISOString(),
	};
	const mounted = mount(() => <Keys />, {
		api: fakeApi().api,
		cached: [
			[["console", "keys", "hashbrown", "active"], { keys: [never], total: 1 }],
		],
	});
	dispose = mounted.dispose;
	const row = mounted.root.querySelector<HTMLElement>("tbody tr");
	expect(row?.innerText).toContain("Never expires");
});

// Kills a key list read while it loads, which suspends the whole page.
test("the page holds while the key lists load", () => {
	dispose = mount(() => <Keys />, { api: fakeApi().api }).dispose;
	expect(
		page.getByRole("heading", { level: 1, name: "Keys" }).elements(),
	).toHaveLength(1);
	expect(page.getByRole("button", { name: "New key" }).elements()).toHaveLength(
		1,
	);
	expect(page.getByText("Suspended").elements()).toHaveLength(0);
});

// Kills a new key dialog that fails axe in either theme.
test("the new key dialog passes axe in both themes", async () => {
	await axeInBothThemes(async () => {
		dispose = mount(() => <Keys />, { api: fakeApi().api, cached }).dispose;
		await page.getByRole("button", { name: "New key" }).click();
		await expect
			.element(page.getByRole("dialog", { name: "New key" }))
			.toBeVisible();
		return () => {
			dispose?.();
			dispose = undefined;
		};
	});
});
