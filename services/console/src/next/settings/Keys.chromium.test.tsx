import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonProjectKey } from "../../types/bencher";
import { ApiError } from "../api";
import { axeInBothThemes } from "../axe/testing";
import { budget } from "../budget";
import { screenBatch } from "../list/rows";
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

// Keyed by the page size the list asks for at the screen's height when it mounts.
const activeKeys = (): [unknown[], unknown] => [
	["console", "keys", "hashbrown", "active", screenBatch(), 1],
	{ rows: [key], total: 1, page: 1 },
];
const cached = (): [unknown[], unknown][] => [
	activeKeys(),
	[
		["console", "keys", "hashbrown", "revoked", screenBatch(), 1],
		{ rows: [], total: 0, page: 1 },
	],
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
	dispose = mount(() => <Keys />, { api, cached: cached() }).dispose;
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
	dispose = mount(() => <Keys />, { api, cached: cached() }).dispose;
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
		cached: cached(),
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
	dispose = mount(() => <Keys />, { api, cached: [activeKeys()] }).dispose;
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
	const mounted = mount(() => <Keys />, { api, cached: cached() });
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
	dispose = mount(() => <Keys />, { api, cached: cached() }).dispose;
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

// Kills a name limit in letters, which lets through a name the API refuses
// for its bytes.
test("a key name is held to the API's 64 bytes", async () => {
	const { api, sent } = fakeApi();
	dispose = mount(() => <Keys />, { api, cached: cached() }).dispose;
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
			[
				["console", "keys", "hashbrown", "active", screenBatch(), 1],
				{ rows: [never], total: 1, page: 1 },
			],
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
		dispose = mount(() => <Keys />, {
			api: fakeApi().api,
			cached: cached(),
		}).dispose;
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

// Kills a list that measures its rows from one opened to its whole name, which
// then draws too few rows to fill the screen.
test("a key opened to its whole name leaves the screen full of rows", async () => {
	// Tall enough that the rows drawn either side cannot hide a short count.
	await page.viewport(390, 3000);
	const keys = Array.from({ length: 200 }, (_, n) => ({
		...key,
		uuid: `uuid-${n}`,
		name: n === 0 ? "k".repeat(64) : `key-${n}`,
	}));
	dispose = mount(
		() => (
			<KeyTable
				status="active"
				label="Active keys"
				empty="No keys"
				keys={keys}
				made={new Set()}
				onRevoke={() => {}}
			/>
		),
		{ api: fakeApi().api },
	).dispose;
	const drawn = () => [
		...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]"),
	];
	await expect.poll(() => drawn().length).toBeGreaterThan(1);
	expect(drawn().length).toBeLessThan(keys.length);
	await expect
		.poll(() => document.querySelector("tbody button.ui-trunc"))
		.not.toBeNull();
	document.querySelector<HTMLElement>("tbody button.ui-trunc")?.click();
	await expect
		.poll(() => drawn()[0]?.getBoundingClientRect().height ?? 0)
		.toBeGreaterThan((drawn()[1]?.getBoundingClientRect().height ?? 0) + 20);
	window.dispatchEvent(new Event("resize"));
	await new Promise((resolve) =>
		requestAnimationFrame(() => setTimeout(resolve, 50)),
	);
	const last = drawn().at(-1)?.getBoundingClientRect().bottom ?? 0;
	expect(last).toBeGreaterThanOrEqual(window.innerHeight);
});

// Kills a page that lists a key twice once a new key shifts the API's pages
// by one, and a list that stops reading once its first page holds one more
// key than a page.
test("after a new key, every key is listed once down to the last page", async () => {
	await page.viewport(1280, 400);
	const made = (n: number, name = `key-${String(n).padStart(4, "0")}`) =>
		({
			...key,
			uuid: `uuid-${n}`,
			name,
			creation: new Date(Date.UTC(2026, 0, 1) + n * 60_000).toISOString(),
		}) as JsonProjectKey;
	const active = Array.from({ length: 60 }, (_, n) => made(n)).reverse();
	const { api } = fakeApi(
		async ({ method }) => {
			if (method !== "POST") {
				return undefined;
			}
			const created = { ...made(60, "fresh"), key: "bencher_secret" };
			active.unshift(created);
			return created;
		},
		async (path) => {
			const url = new URL(path, "http://api.test");
			const size = Number(url.searchParams.get("per_page"));
			const at = Number(url.searchParams.get("page"));
			const list = url.searchParams.get("revoked") === "true" ? [] : active;
			return {
				data: list.slice((at - 1) * size, at * size),
				headers: new Headers({ "X-Total-Count": String(list.length) }),
			} as never;
		},
	);
	dispose = mount(() => <Keys />, { api }).dispose;
	const table = page.getByRole("table", { name: "Active keys" });
	await expect.element(table).toBeVisible();
	await page.getByRole("button", { name: "New key" }).click();
	await page.getByRole("textbox", { name: "Name" }).fill("fresh");
	await page.getByRole("button", { name: "Create key" }).click();
	await page
		.getByRole("dialog", { name: "Copy your new key" })
		.getByRole("button", { name: "Done" })
		.click();
	const seen = new Map<number, string>();
	for (let step = 0; step < 80; step++) {
		for (const row of table
			.element()
			.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]")) {
			seen.set(
				Number(row.getAttribute("aria-rowindex")),
				row.querySelector("b")?.textContent ?? "",
			);
		}
		window.scrollBy({ top: 200, behavior: "instant" });
		await new Promise((resolve) =>
			requestAnimationFrame(() => setTimeout(resolve, 10)),
		);
	}
	window.scrollTo({ top: 0, behavior: "instant" });
	const names = [...seen.entries()]
		.sort(([a], [b]) => a - b)
		.map(([, name]) => name);
	expect(names).toEqual(active.map(({ name }) => name));
});

// Kills a next page that a refused revoke cancels in flight and that never
// asks again, so the keys past it never load.
test("a refused revoke while the next page loads still loads it", async () => {
	await page.viewport(1280, 400);
	const all = Array.from({ length: 60 }, (_, n) => ({
		...key,
		uuid: `uuid-${n}`,
		name: `key-${String(n).padStart(4, "0")}`,
	}));
	const asked: number[] = [];
	const { api } = fakeApi(
		async () => {
			throw new ApiError(500, "server", '{"message":"Database is busy"}');
		},
		async (path) => {
			const url = new URL(path, "http://api.test");
			const size = Number(url.searchParams.get("per_page"));
			const at = Number(url.searchParams.get("page"));
			const list = url.searchParams.get("revoked") === "true" ? [] : all;
			asked.push(at);
			if (at === 2 && asked.filter((each) => each === 2).length === 1) {
				return new Promise<never>(() => {});
			}
			return {
				data: list.slice((at - 1) * size, at * size),
				headers: new Headers({ "X-Total-Count": String(list.length) }),
			} as never;
		},
	);
	dispose = mount(() => <Keys />, { api }).dispose;
	const table = page.getByRole("table", { name: "Active keys" });
	await expect.element(table).toBeVisible();
	for (let step = 0; step < 20 && !asked.includes(2); step++) {
		window.scrollBy({ top: 200, behavior: "instant" });
		await new Promise((resolve) => setTimeout(resolve, 20));
	}
	expect(asked).toContain(2);
	window.scrollTo({ top: 0, behavior: "instant" });
	await page.getByRole("button", { name: "Revoke key-0000" }).click();
	await page.getByRole("button", { name: "Revoke key", exact: true }).click();
	await expect.element(page.getByRole("alert")).toBeVisible();
	await expect.poll(() => asked.filter((each) => each === 2).length).toBe(2);
	for (let step = 0; step < 40; step++) {
		window.scrollBy({ top: 200, behavior: "instant" });
		await new Promise((resolve) => setTimeout(resolve, 20));
	}
	await expect.element(table.getByText("key-0059")).toBeInTheDocument();
	window.scrollTo({ top: 0, behavior: "instant" });
});

/** 1,000 keys, the names at `cut` too long for their cell, at `width`, scrolled by the steps a wheel takes. */
const longList = async (
	width: number,
	cut: readonly number[] = [0, 1],
	count = 1000,
) => {
	await page.viewport(width, 844);
	const keys = Array.from({ length: count }, (_, n) => ({
		...key,
		uuid: `uuid-${n}`,
		name: cut.includes(n) ? `${"k".repeat(60)}-${n}` : `key-${n}`,
	}));
	// A frame's work: from before the list's own scroll handling to after the layout it leaves.
	let start = 0;
	const times: number[] = [];
	const before = () =>
		requestAnimationFrame(() => {
			start = performance.now();
		});
	window.addEventListener("scroll", before, { passive: true });
	dispose = mount(
		() => (
			<KeyTable
				status="active"
				label="Active keys"
				empty="No keys"
				keys={keys}
				made={new Set()}
				onRevoke={() => {}}
			/>
		),
		{ api: fakeApi().api },
	).dispose;
	const after = () =>
		requestAnimationFrame(() => {
			document.body.getBoundingClientRect();
			times.push(performance.now() - start);
		});
	window.addEventListener("scroll", after, { passive: true });
	const frame = () =>
		new Promise<void>((resolve) =>
			requestAnimationFrame(() => setTimeout(resolve, 0)),
		);
	let shift = 0;
	const observer = new PerformanceObserver((list) => {
		for (const entry of list.getEntries()) {
			shift += (entry as unknown as { value: number }).value;
		}
	});
	let most = 0;
	const rows = () => [
		...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]"),
	];
	/** Every drawn row whose index is not its key's. */
	const misplaced = () =>
		rows().filter((row) => {
			const name = row.querySelector("b")?.textContent ?? "";
			const at = Number(name.split("-").at(-1));
			return Number(row.getAttribute("aria-rowindex")) !== at + 2;
		}).length;
	let wrong = 0;
	let watching = false;
	const heights: number[] = [];
	const scrollTo = async (top: number) => {
		window.scrollTo({ top, behavior: "instant" });
		await frame();
		await frame();
		most = Math.max(most, rows().length);
		wrong += misplaced();
		if (watching) {
			heights.push(document.documentElement.scrollHeight);
		}
	};
	// Each step stays inside the rows drawn past the screen, as a wheel's frames do.
	const sweep = async (from: number, to: number) => {
		const step = from < to ? 300 : -300;
		for (let top = from; step > 0 ? top <= to : top >= to; top += step) {
			await scrollTo(top);
		}
	};
	const open = async (at: number) => {
		await expect
			.poll(() => rows()[at]?.querySelector("button.ui-trunc"))
			.toBeTruthy();
		rows()[at]?.querySelector<HTMLElement>("button.ui-trunc")?.click();
		await frame();
		await frame();
	};
	return {
		open,
		sweep,
		scrollTo,
		frame,
		watch: () => {
			watching = true;
			observer.observe({ type: "layout-shift", buffered: false });
		},
		rows,
		end: () => document.documentElement.scrollHeight,
		result: () => {
			observer.disconnect();
			window.removeEventListener("scroll", before);
			window.removeEventListener("scroll", after);
			window.scrollTo({ top: 0, behavior: "instant" });
			const sorted = [...times].sort((a, b) => a - b);
			return {
				shift,
				most,
				wrong,
				median: sorted[Math.floor(sorted.length / 2)] ?? Number.NaN,
				/** How far the page's height moved while watched. */
				drift: Math.max(...heights, 0) - Math.min(...heights, Number.MAX_VALUE),
				opened: rows().filter((row) => row.querySelector("[data-open]")),
			};
		},
	};
};

// Kills two names open at once in one table, a row opened to its whole name
// that draws every row between it and the screen, moves rows on screen when
// it leaves or comes back, or loses its open state, rows after the kept one
// that read another index, and a scroll that takes more than a frame.
test.each([390, 768])(
	"at %i px opening a second name closes the first, and the open row moves nothing as the list scrolls away and back",
	async (width) => {
		const list = await longList(width);
		await list.open(0);
		await list.open(1);
		expect(
			document.querySelectorAll("tbody .ui-trunc[data-open]"),
		).toHaveLength(1);
		list.watch();
		const end = list.end();
		await list.sweep(0, end);
		await list.sweep(end, 0);
		const { shift, most, wrong, median, opened } = list.result();
		expect(opened).toHaveLength(1);
		expect(opened[0]?.getAttribute("aria-rowindex")).toBe("3");
		expect(wrong).toBe(0);
		expect(most).toBeLessThanOrEqual(Math.ceil(844 / 40) + 2 * 8 + 3);
		expect(shift).toBe(0);
		expect(median).toBeLessThan(budget(8.3));
	},
);

// Kills an open row that closes once the reader scrolls up past it, or moves
// rows on screen as it leaves below them and comes back.
test("a key opened far down the list stays open as the reader scrolls up past it and back", async () => {
	const list = await longList(390, [60]);
	await list.scrollTo(3600);
	const far = () =>
		[
			...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]"),
		].findIndex((row) => row.getAttribute("aria-rowindex") === "62");
	expect(far()).toBeGreaterThanOrEqual(0);
	await list.open(far());
	list.watch();
	await list.sweep(3600, 0);
	await list.sweep(0, 3600);
	const { shift, wrong, opened, drift } = list.result();
	expect(opened.map((row) => row.getAttribute("aria-rowindex"))).toEqual([
		"62",
	]);
	expect(wrong).toBe(0);
	expect(shift).toBe(0);
	expect(drift).toBeLessThan(1);
});

// Kills a row step taken from the first row, which a folded table draws a
// pixel short, so every spacer runs short until it scrolls away and the
// page's height jumps.
test("the list's height holds as its first row scrolls away", async () => {
	const list = await longList(390, []);
	await list.frame();
	const top = list.end();
	await list.scrollTo(2000);
	expect(Math.abs(list.end() - top)).toBeLessThan(1);
	list.result();
});

// Kills a row step measured anew from rows a browser snaps to heights half a
// pixel apart, as WebKit does, which every spacer multiplies into a jump.
test("the list's height holds as it scrolls past rows half a pixel apart", async () => {
	const list = await longList(768, []);
	await list.frame();
	const height = list.rows()[1]?.getBoundingClientRect().height ?? 0;
	const style = document.createElement("style");
	style.textContent = `tbody tr[aria-rowindex]:nth-child(3n) { height: ${height + 0.5}px; }`;
	document.head.append(style);
	await list.frame();
	list.watch();
	await list.sweep(0, 12_000);
	style.remove();
	expect(list.result().drift).toBeLessThan(10);
});

// Kills a row height held from before a web font lands, which leaves every
// spacer short by the font's change for the life of the list.
test("the list's height follows a web font that lands", async () => {
	const list = await longList(768, []);
	await document.fonts.ready;
	await list.frame();
	await list.frame();
	const top = list.end();
	const height = list.rows()[1]?.getBoundingClientRect().height ?? 0;
	const style = document.createElement("style");
	style.textContent = `tbody tr[aria-rowindex] { height: ${height + 0.5}px; }`;
	document.head.append(style);
	document.fonts.dispatchEvent(new Event("loadingdone"));
	await list.frame();
	await list.frame();
	const end = list.end();
	style.remove();
	list.result();
	expect(Math.abs(end - top - 500)).toBeLessThan(2);
});

// Kills a web font that lands while the reader is scrolled down, whose
// re-measure grows every spacer above the screen and moves the rows on it.
test.each([0, 500])(
	"a web font that lands with row %i at the top moves nothing on screen",
	async (at) => {
		const list = await longList(768, []);
		await document.fonts.ready;
		await list.frame();
		const height = list.rows()[1]?.getBoundingClientRect().height ?? 0;
		await list.scrollTo(at * height);
		const row = list
			.rows()
			.find((each) => each.getBoundingClientRect().bottom > 0);
		const top = row?.getBoundingClientRect().top ?? 0;
		const style = document.createElement("style");
		style.textContent = `tbody tr[aria-rowindex] { height: ${height + 0.5}px; }`;
		document.head.append(style);
		document.fonts.dispatchEvent(new Event("loadingdone"));
		await list.frame();
		await list.frame();
		const moved = (row?.getBoundingClientRect().top ?? 0) - top;
		style.remove();
		expect(row?.isConnected).toBe(true);
		expect(Math.abs(moved)).toBeLessThan(1);
		if (at === 0) {
			expect(window.scrollY).toBe(0);
		}
		list.result();
	},
);

// Kills a web font that shortens the rows deep in a long list, whose
// re-measure draws other rows in place of the marked one, so nothing holds.
test("a web font that shortens the rows deep in a long list moves nothing on screen", async () => {
	const list = await longList(768, [], 3000);
	await document.fonts.ready;
	await list.frame();
	const height = (list.rows()[1]?.getBoundingClientRect().height ?? 0) + 1;
	const style = document.createElement("style");
	style.textContent = `tbody tr[aria-rowindex] { height: ${height}px; }`;
	document.head.append(style);
	document.fonts.dispatchEvent(new Event("loadingdone"));
	await list.frame();
	await list.scrollTo(2500 * height);
	const first = () =>
		list.rows().find((each) => each.getBoundingClientRect().bottom > 0);
	const index = first()?.getAttribute("aria-rowindex");
	const top = first()?.getBoundingClientRect().top ?? 0;
	style.textContent = `tbody tr[aria-rowindex] { height: ${height - 0.5}px; }`;
	document.fonts.dispatchEvent(new Event("loadingdone"));
	await list.frame();
	await list.frame();
	const row = first();
	const moved = (row?.getBoundingClientRect().top ?? 0) - top;
	style.remove();
	list.result();
	expect(row?.getAttribute("aria-rowindex")).toBe(index);
	expect(Math.abs(moved)).toBeLessThan(1);
});

// Kills a web font that lands in the same task as a scroll, before a frame
// has measured the rows the scroll brought to the screen.
test("a web font that lands with a scroll holds the row the scroll brought to the top", async () => {
	const list = await longList(768, []);
	await document.fonts.ready;
	await list.frame();
	const height = list.rows()[1]?.getBoundingClientRect().height ?? 0;
	await list.scrollTo(500 * height);
	window.scrollBy({ top: 300, behavior: "instant" });
	const row = list
		.rows()
		.find((each) => each.getBoundingClientRect().bottom > 0);
	const top = row?.getBoundingClientRect().top ?? 0;
	const style = document.createElement("style");
	style.textContent = `tbody tr[aria-rowindex] { height: ${height + 0.5}px; }`;
	document.head.append(style);
	document.fonts.dispatchEvent(new Event("loadingdone"));
	await list.frame();
	await list.frame();
	const moved = (row?.getBoundingClientRect().top ?? 0) - top;
	style.remove();
	list.result();
	expect(row?.isConnected).toBe(true);
	expect(Math.abs(moved)).toBeLessThan(1);
});

/** Scroll until the drawn row for key `at` is on screen, and where it is among the drawn rows. */
const reach = async (
	list: Awaited<ReturnType<typeof longList>>,
	at: number,
) => {
	const index = () =>
		list
			.rows()
			.findIndex((row) => row.getAttribute("aria-rowindex") === String(at + 2));
	for (let top = 0; top < 100_000; top += 300) {
		await list.scrollTo(top);
		const row = list.rows()[index()];
		const box = row?.getBoundingClientRect();
		if (box && box.top > 200 && box.bottom < 600) {
			return index;
		}
	}
	throw new Error(`key ${at} never came on screen`);
};

// Kills a second name opened far down the list whose close of the first, kept
// above the screen, moves the row pressed on screen. The browser's scroll
// anchoring holds the row in place; the shift it reports follows a press, which
// the page's layout shift score leaves out (keys-scroll.e2e.ts; a test frame
// here never sees the press as input).
test.each([390, 768])(
	"at %i px opening a name far down the list, which closes the one kept above the screen, moves nothing",
	async (width) => {
		const list = await longList(width, [0, 300]);
		await list.open(0);
		const index = await reach(list, 300);
		const top = list.rows()[index()]?.getBoundingClientRect().top ?? 0;
		list.watch();
		const reveal = list
			.rows()
			[index()]?.querySelector<HTMLElement>("button.ui-trunc") as HTMLElement;
		reveal.focus();
		await userEvent.keyboard("{Enter}");
		await list.frame();
		await list.frame();
		const row = list.rows()[index()];
		expect(row?.querySelector("[data-open]")).not.toBeNull();
		expect(list.rows()[0]?.querySelector("[data-open]")).toBeNull();
		expect(
			Math.abs((row?.getBoundingClientRect().top ?? 0) - top),
		).toBeLessThan(1);
		list.result();
	},
);

// Kills a press anywhere in a row, such as its Revoke, read as the close of
// the name kept open, which then leaves its place and moves the rows on screen.
test("a Revoke press leaves the name kept open where it is", async () => {
	const list = await longList(390, [0]);
	await list.open(0);
	const index = await reach(list, 200);
	list
		.rows()
		[index()]?.querySelector<HTMLElement>("button.lnk.danger")
		?.click();
	await list.frame();
	await list.frame();
	expect(list.rows()[0]?.querySelector("[data-open]")).not.toBeNull();
	list.result();
});

// Kills an opened name that loses the focus when its row moves to the kept
// slot below the screen, or back among the rows near it.
test("an opened name keeps the focus as the reader scrolls up past it and back", async () => {
	const list = await longList(390, [300]);
	const index = await reach(list, 300);
	const reveal = list
		.rows()
		[index()]?.querySelector<HTMLElement>("button.ui-trunc") as HTMLElement;
	reveal.focus();
	reveal.click();
	await list.frame();
	await list.frame();
	const y = window.scrollY;
	await list.sweep(y, 0);
	expect(document.activeElement?.textContent).toBe(`${"k".repeat(60)}-300`);
	await list.sweep(0, y);
	expect(document.activeElement?.textContent).toBe(`${"k".repeat(60)}-300`);
	list.result();
});
