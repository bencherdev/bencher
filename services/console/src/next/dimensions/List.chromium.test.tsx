import "./dimensions.css";
import { focusManager } from "@tanstack/solid-query";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonConsoleBranchRow } from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { type Sent, bootstrapOf, mount } from "../settings/testing";
import List from "./List";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	window.scrollTo({ top: 0, behavior: "instant" });
});

const DAY = 24 * 60 * 60 * 1_000;
const NOW = Date.parse("2026-09-14T00:00:00Z");

const row = (
	name: string,
	fields: Partial<JsonConsoleBranchRow> = {},
): JsonConsoleBranchRow => ({
	uuid: `uuid-${name}`,
	name,
	slug: name,
	created: NOW - 30 * DAY,
	last_report: NOW - DAY,
	thresholds: 0,
	held_thresholds: 0,
	...fields,
});

const ACTIVE = [
	row("devel"),
	row("feature-simd", { thresholds: 1, start_point: "main" }),
	row("main", { thresholds: 2 }),
];

/** An API whose lists answer from `lists` and whose changes wait for `answer`. */
const api = ({
	lists = (search: URLSearchParams) =>
		search.get("archived") === "true"
			? [
					row("gone", {
						archived: NOW - 2 * DAY,
						thresholds: 1,
						held_thresholds: 1,
					}),
				]
			: ACTIVE,
	answer = async () => undefined,
	hold = () => Promise.resolve(),
}: {
	lists?: (search: URLSearchParams) => JsonConsoleBranchRow[];
	answer?: (sent: Sent) => Promise<unknown>;
	/** What a list read waits for before it answers. */
	hold?: (search: URLSearchParams) => Promise<void>;
} = {}) => {
	const sent: Sent[] = [];
	const reads: URL[] = [];
	const fake: Api = {
		get: async <T,>(path: string) => {
			const url = new URL(path, "http://api.test");
			reads.push(url);
			await hold(url.searchParams);
			const branches = lists(url.searchParams);
			return {
				data: {
					total: branches.length,
					active: 3,
					archived: 1,
					branches,
				} as T,
				headers: new Headers(),
			};
		},
		send: async <T,>(method: Sent["method"], path: string, body?: unknown) => {
			const change = { method, path, body };
			sent.push(change);
			return { data: (await answer(change)) as T, headers: new Headers() };
		},
	};
	return { api: fake, sent, reads };
};

const show = (fake: Api, { edit = true, path = "/hashbrown/branches" } = {}) =>
	mount(() => <List dimension="branches" perPage={26} edit={edit} />, {
		api: fake,
		path,
		bootstrap: bootstrapOf({ edit }),
	});

const branches = () => page.getByRole("table", { name: "Active branches" });
const rowOf = (name: string) =>
	branches()
		.getByRole("row")
		.filter({ has: page.getByRole("link", { name, exact: true }) });

// Kills an archive that waits for the API before the row changes, totals
// that wait too, and a refused archive left showing as archived.
test("archiving dims the row at once, and a refusal puts it back and says why", async () => {
	let refuse: (error: unknown) => void = () => {};
	const fake = api({
		answer: () =>
			new Promise((_, reject) => {
				refuse = reject;
			}),
	});
	dispose = show(fake.api).dispose;

	await page.getByRole("button", { name: "Archive feature-simd" }).click();
	const impact = page.getByRole("group", { name: "Archive feature-simd" });
	await expect
		.element(impact)
		.toHaveTextContent(
			"Archiving feature-simd archives 1 threshold and hides its lines.",
		);
	await impact.getByRole("button", { name: "Archive feature-simd" }).click();

	await expect
		.element(rowOf("feature-simd"))
		.toHaveTextContent(/Archived just now/);
	await expect
		.element(page.getByRole("radio", { name: "Active 2" }))
		.toBeInTheDocument();
	await expect
		.element(page.getByRole("status"))
		.toHaveTextContent("Archived feature-simd with its 1 threshold.");
	await expect
		.poll(() => fake.sent)
		.toEqual([
			{
				method: "PATCH",
				path: "/v0/projects/hashbrown/branches/uuid-feature-simd",
				body: { archived: true },
			},
		]);

	refuse(new ApiError(409, "client", '{"message":"Busy"}'));
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not archive feature-simd: Busy");
	await expect
		.element(
			rowOf("feature-simd").getByRole("button", {
				name: "Archive feature-simd",
			}),
		)
		.toBeVisible();
	await expect
		.element(page.getByRole("radio", { name: "Active 3" }))
		.toBeInTheDocument();
});

// Kills an Undo sent before the archive it undoes, which the API would apply
// in the wrong order and leave the branch archived.
test("an Undo before the archive lands is sent after it", async () => {
	const answers: (() => void)[] = [];
	const fake = api({
		answer: () =>
			new Promise((resolve) => {
				answers.push(() => resolve(undefined));
			}),
	});
	dispose = show(fake.api).dispose;

	await page.getByRole("button", { name: "Archive devel" }).click();
	await page
		.getByRole("group", { name: "Archive devel" })
		.getByRole("button", { name: "Archive devel" })
		.click();
	// The confirm left with the impact line; the focus moves to the row's Undo.
	await expect
		.poll(() => document.activeElement?.getAttribute("aria-label"))
		.toBe("Undo, devel");
	await page.getByRole("button", { name: "Undo, devel" }).click();
	await expect
		.element(rowOf("devel").getByRole("button", { name: "Archive devel" }))
		.toBeVisible();
	await expect.poll(() => fake.sent.length).toBe(1);

	answers[0]?.();
	await expect
		.poll(() => fake.sent.map(({ body }) => body))
		.toEqual([{ archived: true }, { archived: false }]);
	answers[1]?.();
	await expect
		.element(page.getByRole("radio", { name: "Active 3" }))
		.toBeInTheDocument();
});

// Kills an Unarchive that claims every threshold back while another
// dimension holds one.
test("unarchiving says which thresholds come back and which stay", async () => {
	const fake = api();
	dispose = show(fake.api, {
		path: "/hashbrown/branches?archived=true",
	}).dispose;
	await page.getByRole("button", { name: "Unarchive gone" }).click();
	await expect
		.element(page.getByRole("status"))
		.toHaveTextContent(
			"Unarchived gone and 1 threshold. One threshold stays archived: another of its dimensions is archived.",
		);
	await expect
		.element(page.getByRole("button", { name: "Undo, gone" }))
		.toBeVisible();
	expect(fake.sent.map(({ body }) => body)).toEqual([{ archived: false }]);
});

// Kills Archive or Unarchive drawn for a reader the API would refuse.
test("a reader who cannot edit sees when a row was archived, and no control", async () => {
	const fake = api();
	dispose = show(fake.api, { edit: false }).dispose;
	await expect.element(rowOf("main")).toBeVisible();
	expect(page.getByRole("button", { name: /Archive/ }).elements()).toEqual([]);

	await page.getByRole("radio", { name: /^Archived/ }).click();
	await expect
		.element(page.getByRole("table", { name: "Archived branches" }))
		.toHaveTextContent(/Archived Sep 12/);
	expect(page.getByRole("button", { name: /Unarchive/ }).elements()).toEqual(
		[],
	);
});

// Kills a refine that drops the rows on screen for a skeleton.
test("the rows stay, marked busy, until the other status answers", async () => {
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const fake = api({
		hold: (search) =>
			search.get("archived") === "true" ? held : Promise.resolve(),
	});
	dispose = show(fake.api).dispose;
	await expect.element(rowOf("main")).toBeVisible();

	await page.getByRole("radio", { name: /^Archived/ }).click();
	const kept = page.getByRole("table", { name: "Archived branches" });
	await expect.element(kept).toHaveAttribute("aria-busy", "true");
	await expect.element(kept.getByRole("link", { name: "main" })).toBeVisible();

	release();
	await expect.element(kept.getByRole("link", { name: "gone" })).toBeVisible();
	await expect.element(kept).not.toHaveAttribute("aria-busy");
});

// Kills a list that draws every row it holds instead of the rows in view.
test("a long list draws only the rows on screen", async () => {
	const many = Array.from({ length: 300 }, (_, index) =>
		row(`branch-${String(index).padStart(3, "0")}`),
	);
	await page.viewport(1280, 720);
	const fake = api({ lists: () => many });
	dispose = mount(
		() => <List dimension="branches" perPage={300} edit={true} />,
		{ api: fake.api, path: "/hashbrown/branches" },
	).dispose;
	await expect.element(rowOf("branch-000")).toBeVisible();
	const drawn = branches().getByRole("link").elements().length;
	expect(drawn).toBeGreaterThan(5);
	expect(drawn).toBeLessThan(60);

	const body = document.querySelector("tbody") as HTMLElement;
	window.scrollTo({
		top: body.getBoundingClientRect().top + 280 * 40,
		behavior: "instant",
	});
	await expect
		.element(branches().getByRole("link", { name: "branch-280" }))
		.toBeInTheDocument();
	expect(
		branches().getByRole("link", { name: "branch-000" }).elements(),
	).toEqual([]);
});

// Kills a filter sent on every keystroke, or never, and a filter that finds
// nothing without saying so or offering the way back.
test("the filter asks once the typing stops, and an empty answer offers to clear it", async () => {
	const fake = api({
		lists: (search) => (search.get("search") ? [] : ACTIVE),
	});
	dispose = show(fake.api).dispose;
	await expect.element(rowOf("main")).toBeVisible();
	const before = fake.reads.length;

	await userEvent.type(
		page.getByRole("searchbox", { name: "Filter branches" }),
		"simd",
	);
	await expect
		.element(page.getByText('No branches match "simd".'))
		.toBeVisible();
	const searched = fake.reads
		.slice(before)
		.map((url) => url.searchParams.get("search"));
	expect(searched).toEqual(["simd"]);

	await page.getByRole("button", { name: "Clear the filter" }).click();
	await expect.element(rowOf("main")).toBeVisible();
	await expect
		.element(page.getByRole("searchbox", { name: "Filter branches" }))
		.toHaveValue("");
});

// Kills an archive that refetches the list on screen once it lands, which
// would drop the dimmed row and its Undo, and one that leaves the rest of the
// project stale.
test("a landed archive keeps its row dimmed with Undo, with no new read of the list", async () => {
	const fake = api();
	const mounted = show(fake.api);
	dispose = mounted.dispose;
	await expect.element(rowOf("main")).toBeVisible();
	const before = fake.reads.length;
	await page.getByRole("button", { name: "Archive main" }).click();
	await page
		.getByRole("group", { name: "Archive main" })
		.getByRole("button", { name: "Archive main" })
		.click();
	await expect.poll(() => fake.sent.length).toBe(1);
	// The change has landed and the page has had its chance to refetch.
	await new Promise((resolve) => setTimeout(resolve, 200));
	await expect
		.element(rowOf("main").getByRole("button", { name: "Undo, main" }))
		.toBeVisible();
	expect(fake.reads.slice(before)).toEqual([]);
	// The rest of the project, such as the shell's alert count, is stale.
	expect(
		mounted.client.getQueryState(["console", "project", "hashbrown"])
			?.isInvalidated,
	).toBe(true);
});

// Kills a list that stops at its first batch, and one that asks for the next
// batch before the reader scrolls toward it.
test("the next batch loads as the reader nears the end", async () => {
	await page.viewport(1280, 720);
	const many = Array.from({ length: 60 }, (_, index) =>
		row(`branch-${String(index).padStart(3, "0")}`),
	);
	const reads: URL[] = [];
	const fake: Api = {
		get: async <T,>(path: string) => {
			const url = new URL(path, "http://api.test");
			reads.push(url);
			const offset = Number(url.searchParams.get("offset"));
			const perPage = Number(url.searchParams.get("per_page"));
			return {
				data: {
					total: many.length,
					active: many.length,
					archived: 0,
					branches: many.slice(offset, offset + perPage),
				} as T,
				headers: new Headers(),
			};
		},
		send: async () => {
			throw new Error("no changes");
		},
	};
	dispose = mount(
		() => <List dimension="branches" perPage={26} edit={true} />,
		{ api: fake, path: "/hashbrown/branches" },
	).dispose;
	await expect.element(rowOf("branch-000")).toBeVisible();
	expect(reads.map((url) => url.searchParams.get("offset"))).toEqual(["0"]);

	const body = document.querySelector("tbody") as HTMLElement;
	window.scrollTo({
		top: body.getBoundingClientRect().top + 26 * 40,
		behavior: "instant",
	});
	await expect
		.element(branches().getByRole("link", { name: "branch-030" }))
		.toBeInTheDocument();
	expect(reads.map((url) => url.searchParams.get("offset"))).toEqual([
		"0",
		"26",
	]);
});

const numbered = (count: number) =>
	Array.from({ length: count }, (_, index) =>
		row(`branch-${String(index).padStart(3, "0")}`),
	);

/**
 * An API that pages its active rows as the console list does, by offset or
 * else by page, and applies each change once `hold` lets it through.
 */
const pagingApi = (
	all: JsonConsoleBranchRow[],
	{
		hold = () => Promise.resolve(),
		wait = () => Promise.resolve(),
		shift = 0,
		fail = () => false,
	}: {
		hold?: (sent: Sent) => Promise<void>;
		/** What a read at this offset waits for before it answers. */
		wait?: (offset: number) => Promise<void>;
		/** Rows each later batch starts early, as when another reader unarchives one. */
		shift?: number;
		/** Whether a read at this offset fails. */
		fail?: (offset: number) => boolean;
	} = {},
) => {
	const archived = new Set<string>();
	const reads: URL[] = [];
	const sent: Sent[] = [];
	const fake: Api = {
		get: async <T,>(path: string) => {
			const url = new URL(path, "http://api.test");
			reads.push(url);
			const perPage = Number(url.searchParams.get("per_page"));
			const asked = url.searchParams.has("offset")
				? Number(url.searchParams.get("offset"))
				: (Number(url.searchParams.get("page") ?? "1") - 1) * perPage;
			await wait(asked);
			if (fail(asked)) {
				throw new ApiError(503, "server", "{}");
			}
			const offset = asked > 0 ? asked - shift : 0;
			const active = all.filter((each) => !archived.has(each.uuid));
			return {
				data: {
					total: active.length,
					active: active.length,
					archived: archived.size,
					branches: active.slice(offset, offset + perPage),
				} as T,
				headers: new Headers(),
			};
		},
		send: async <T,>(method: Sent["method"], path: string, body?: unknown) => {
			const change = { method, path, body };
			sent.push(change);
			await hold(change);
			const uuid = path.split("/").at(-1) ?? "";
			if ((body as { archived: boolean }).archived) {
				archived.add(uuid);
			} else {
				archived.delete(uuid);
			}
			return { data: undefined as T, headers: new Headers() };
		},
	};
	return { api: fake, archived, reads, sent };
};

const offsets = (reads: URL[]) =>
	reads.map((url) => url.searchParams.get("offset"));

/** Scroll the list so the row at `index` sits near the top of the screen. */
const scrollToRow = (index: number) => {
	const body = document.querySelector("tbody") as HTMLElement;
	window.scrollTo({
		top: body.getBoundingClientRect().top + window.scrollY + index * 40 - 80,
		behavior: "instant",
	});
};

const archiveRow = async (name: string) => {
	await page.getByRole("button", { name: `Archive ${name}` }).click();
	await page
		.getByRole("group", { name: `Archive ${name}` })
		.getByRole("button", { name: `Archive ${name}` })
		.click();
};

const drawn = (name: string) =>
	branches().getByRole("link", { name, exact: true }).elements().length;

/** Scroll toward the row at `index`, as the batches on the way load, until `name` is drawn once. */
const scrollUntilDrawn = (index: number, name: string) =>
	expect
		.poll(() => {
			scrollToRow(index);
			return drawn(name);
		})
		.toBe(1);

// Kills a next batch asked for by page, which skips one row for each row
// archived on screen, and a list that stops once its drawn rows, dimmed ones
// too, reach the total.
test("after archiving on screen, the next batches bring every row once, none skipped", async () => {
	await page.viewport(1280, 720);
	const all = numbered(60);
	const fake = pagingApi(all);
	dispose = show(fake.api).dispose;
	await expect.element(rowOf("branch-000")).toBeVisible();
	const archived = all.slice(1, 11).map(({ name }) => name);
	for (const name of archived) {
		await archiveRow(name);
	}
	await expect.poll(() => fake.archived.size).toBe(archived.length);

	for (const [index, { name }] of all.entries()) {
		await scrollUntilDrawn(index, name);
	}
	expect(offsets(fake.reads)).toEqual(["0", "16", "42"]);
});

// Kills a next batch asked for while a change is on its way: an Undo shown
// at once but not yet applied would make the offset skip a row.
test("the next batch waits for a change on its way, then starts after the rows still in view", async () => {
	await page.viewport(1280, 720);
	let release = () => {};
	const fake = pagingApi(numbered(60), {
		hold: (sent) =>
			(sent.body as { archived: boolean }).archived
				? Promise.resolve()
				: new Promise((resolve) => {
						release = resolve;
					}),
	});
	dispose = show(fake.api).dispose;
	await expect.element(rowOf("branch-000")).toBeVisible();
	await archiveRow("branch-001");
	await expect.poll(() => fake.archived.size).toBe(1);
	await page.getByRole("button", { name: "Undo, branch-001" }).click();
	await expect.poll(() => fake.sent.length).toBe(2);

	scrollToRow(25);
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(offsets(fake.reads)).toEqual(["0"]);
	release();
	await expect
		.element(branches().getByRole("link", { name: "branch-026" }))
		.toBeInTheDocument();
	expect(offsets(fake.reads)).toEqual(["0", "26"]);
	expect(drawn("branch-025")).toBe(1);
});

// Kills a next batch cancelled by a change and never asked for again, which
// left the list without its later rows.
test("a change while the next batch loads asks for that batch again, after the change", async () => {
	await page.viewport(1280, 720);
	let release = () => {};
	const gate = new Promise<void>((resolve) => {
		release = resolve;
	});
	const fake = pagingApi(numbered(60), {
		wait: (offset) => (offset > 0 ? gate : Promise.resolve()),
	});
	dispose = show(fake.api).dispose;
	await expect.element(rowOf("branch-000")).toBeVisible();
	scrollToRow(25);
	await expect.poll(() => offsets(fake.reads)).toEqual(["0", "26"]);
	await archiveRow("branch-020");
	await expect.poll(() => fake.archived.size).toBe(1);
	release();
	await scrollUntilDrawn(26, "branch-026");
	expect(offsets(fake.reads)).toEqual(["0", "26", "25"]);
});

// Kills a row drawn twice when the API's rows shift between batches, as when
// another reader unarchives one.
test("a row that two batches both hold is drawn once", async () => {
	await page.viewport(1280, 720);
	const fake = pagingApi(numbered(60), { shift: 1 });
	dispose = show(fake.api).dispose;
	await expect.element(rowOf("branch-000")).toBeVisible();
	scrollToRow(22);
	await expect
		.element(branches().getByRole("link", { name: "branch-030" }))
		.toBeInTheDocument();
	expect(drawn("branch-025")).toBe(1);
});

// Kills a row archived in place that returning to the window takes away with
// its Undo, once the list's fresh answer leaves it out; a row count that drops
// it; and an Undo there that moves no total, the cache no longer holding the
// row.
test("a row archived in place keeps its place and its Undo when returning to the window reads the list again", async () => {
	await page.viewport(1280, 720);
	const all = numbered(60);
	const fake = pagingApi(all);
	dispose = show(fake.api).dispose;
	await expect.element(rowOf("branch-000")).toBeVisible();
	await archiveRow("branch-001");
	await expect.poll(() => fake.archived.size).toBe(1);
	const before = fake.reads.length;
	focusManager.setFocused(false);
	focusManager.setFocused(true);
	try {
		await expect.poll(() => fake.reads.length).toBeGreaterThan(before);
		await new Promise((resolve) => setTimeout(resolve, 200));
	} finally {
		focusManager.setFocused(undefined);
	}

	await expect
		.element(rowOf("branch-001"))
		.toHaveTextContent(/Archived just now/);
	await expect
		.element(rowOf("branch-001"))
		.toHaveAttribute("aria-rowindex", "3");
	await expect.element(branches()).toHaveAttribute("aria-rowcount", "61");
	for (const [index, { name }] of all.entries()) {
		await scrollUntilDrawn(index, name);
	}
	scrollToRow(0);
	await page.getByRole("button", { name: "Undo, branch-001" }).click();
	await expect
		.element(page.getByRole("radio", { name: "Active 60" }))
		.toBeInTheDocument();
	await expect.poll(() => fake.archived.size).toBe(0);
});

// Kills a row changed in place drawn in a view it was not changed in, which
// the API's answer for that view leaves out.
test("a row archived in place is drawn only under the filter it was archived under", async () => {
	const fake = api({
		lists: (search) =>
			search.get("search") === "simd" ? [row("feature-simd")] : ACTIVE,
	});
	dispose = show(fake.api).dispose;
	await archiveRow("devel");
	await expect.poll(() => fake.sent.length).toBe(1);
	await userEvent.type(
		page.getByRole("searchbox", { name: "Filter branches" }),
		"simd",
	);
	await expect.element(rowOf("main")).not.toBeInTheDocument();
	await expect.element(rowOf("feature-simd")).toBeVisible();
	expect(drawn("devel")).toBe(0);
});

// Kills a refused Undo on a row the list was read again without that leaves
// the totals where the Undo moved them: the cache no longer holds the row, so
// they move back from the state the row was shown in.
test("under Archived, a refused Undo on a row the list was read again without puts the totals back", async () => {
	const names = ["old-1", "old-2", "old-3"];
	const archived = new Set(names.map((name) => `uuid-${name}`));
	let refuse = false;
	const reads: URL[] = [];
	const fake: Api = {
		get: async <T,>(path: string) => {
			const url = new URL(path, "http://api.test");
			reads.push(url);
			const listed = url.searchParams.get("archived") === "true";
			const branches = names
				.filter((name) => archived.has(`uuid-${name}`) === listed)
				.map((name) => row(name, listed ? { archived: NOW - 2 * DAY } : {}));
			return {
				data: {
					total: branches.length,
					active: names.length - archived.size,
					archived: archived.size,
					branches,
				} as T,
				headers: new Headers(),
			};
		},
		send: async <T,>(_method: Sent["method"], path: string, body?: unknown) => {
			if (refuse) {
				throw new ApiError(409, "client", '{"message":"Busy"}');
			}
			const uuid = path.split("/").at(-1) ?? "";
			if ((body as { archived: boolean }).archived) {
				archived.add(uuid);
			} else {
				archived.delete(uuid);
			}
			return { data: undefined as T, headers: new Headers() };
		},
	};
	dispose = show(fake, { path: "/hashbrown/branches?archived=true" }).dispose;
	const table = page.getByRole("table", { name: "Archived branches" });
	await page.getByRole("button", { name: "Unarchive old-2" }).click();
	await expect.poll(() => archived.size).toBe(2);
	const before = reads.length;
	focusManager.setFocused(false);
	focusManager.setFocused(true);
	try {
		await expect.poll(() => reads.length).toBeGreaterThan(before);
		await new Promise((resolve) => setTimeout(resolve, 200));
	} finally {
		focusManager.setFocused(undefined);
	}
	await expect
		.element(table.getByRole("button", { name: "Undo, old-2" }))
		.toBeVisible();

	refuse = true;
	await table.getByRole("button", { name: "Undo, old-2" }).click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not archive old-2: Busy");
	await expect
		.element(page.getByRole("radio", { name: "Archived 2" }))
		.toBeInTheDocument();
	await expect
		.element(page.getByRole("radio", { name: "Active 1" }))
		.toBeInTheDocument();
	await expect.element(table).toHaveTextContent(/Unarchived just now/);
});

// Kills a refusal that rolls a row back over the Undo already queued after
// it, which left the totals at "Active 4" and "Archived 0" over three rows.
test("a refused archive whose Undo is queued leaves the row and the totals as the reader left them", async () => {
	const pending: { resolve: () => void; reject: (error: unknown) => void }[] =
		[];
	const fake = api({
		answer: () =>
			new Promise((resolve, reject) => {
				pending.push({ resolve: () => resolve(undefined), reject });
			}),
	});
	dispose = show(fake.api).dispose;
	await archiveRow("devel");
	await expect
		.element(page.getByRole("radio", { name: "Active 2" }))
		.toBeInTheDocument();
	await page.getByRole("button", { name: "Undo, devel" }).click();
	await expect.poll(() => fake.sent.length).toBe(1);

	pending[0]?.reject(new ApiError(409, "client", '{"message":"Busy"}'));
	await expect.poll(() => fake.sent.length).toBe(2);
	pending[1]?.resolve();
	await expect
		.element(page.getByRole("radio", { name: "Active 3" }))
		.toBeInTheDocument();
	await expect
		.element(page.getByRole("radio", { name: "Archived 1" }))
		.toBeInTheDocument();
	await expect
		.element(rowOf("devel").getByRole("button", { name: "Archive devel" }))
		.toBeVisible();
});

// Kills a refusal that rolls its row back while a later change on the row is
// queued, which left the row active once that later archive landed.
test("a refused archive leaves the row to the later changes queued on it", async () => {
	const pending: { resolve: () => void; reject: (error: unknown) => void }[] =
		[];
	const fake = api({
		answer: () =>
			new Promise((resolve, reject) => {
				pending.push({ resolve: () => resolve(undefined), reject });
			}),
	});
	dispose = show(fake.api).dispose;
	await archiveRow("devel");
	await page.getByRole("button", { name: "Undo, devel" }).click();
	await archiveRow("devel");
	await expect.poll(() => fake.sent.length).toBe(1);

	pending[0]?.reject(new ApiError(409, "client", '{"message":"Busy"}'));
	await expect.poll(() => fake.sent.length).toBe(2);
	pending[1]?.resolve();
	await expect.poll(() => fake.sent.length).toBe(3);
	pending[2]?.resolve();
	await expect.element(rowOf("devel")).toHaveTextContent(/Archived just now/);
	await expect
		.element(page.getByRole("radio", { name: "Active 2" }))
		.toBeInTheDocument();
	await expect
		.element(page.getByRole("radio", { name: "Archived 2" }))
		.toBeInTheDocument();
});

// Kills a refused Undo that puts the row back as it was before the archive
// rather than as the API last confirmed it.
test("a refused Undo after the archive landed shows the row archived again", async () => {
	let calls = 0;
	const fake = api({
		answer: async () => {
			calls += 1;
			if (calls === 2) {
				throw new ApiError(409, "client", '{"message":"Busy"}');
			}
		},
	});
	dispose = show(fake.api).dispose;
	await archiveRow("devel");
	await expect.poll(() => fake.sent.length).toBe(1);
	await page.getByRole("button", { name: "Undo, devel" }).click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not unarchive devel: Busy");
	await expect.element(rowOf("devel")).toHaveTextContent(/Archived just now/);
	await expect
		.element(page.getByRole("radio", { name: "Active 2" }))
		.toBeInTheDocument();
	await expect
		.element(page.getByRole("radio", { name: "Archived 2" }))
		.toBeInTheDocument();
});

// Kills an impact line that leaves the focus on Archive, an Escape it
// ignores, and an Undo that drops the focus with the button it replaces.
test("the focus follows the row through its impact line, Escape, and Undo", async () => {
	const fake = api();
	dispose = show(fake.api).dispose;
	const focused = () =>
		document.activeElement?.getAttribute("aria-label") ??
		document.activeElement?.textContent;

	await page.getByRole("button", { name: "Archive main" }).click();
	await expect.poll(focused).toBe("Archive main");
	expect(
		document.activeElement?.closest("fieldset")?.getAttribute("aria-label"),
	).toBe("Archive main");
	await userEvent.keyboard("{Escape}");
	await expect
		.poll(() => page.getByRole("group", { name: "Archive main" }).elements())
		.toEqual([]);
	await expect.poll(focused).toBe("Archive main");
	expect(document.activeElement?.closest("fieldset")).toBeNull();

	await archiveRow("main");
	await expect.poll(focused).toBe("Undo, main");
	await page.getByRole("button", { name: "Undo, main" }).click();
	await expect.poll(focused).toBe("Archive main");
});

// Kills an impact line left open over the other status's rows while they
// load.
test("switching status closes an open impact line", async () => {
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const fake = api({
		hold: (search) =>
			search.get("archived") === "true" ? held : Promise.resolve(),
	});
	dispose = show(fake.api).dispose;
	await page.getByRole("button", { name: "Archive main" }).click();
	await expect
		.element(page.getByRole("group", { name: "Archive main" }))
		.toBeVisible();
	await page.getByRole("radio", { name: /^Archived/ }).click();
	await expect
		.element(page.getByRole("table", { name: "Archived branches" }))
		.toHaveAttribute("aria-busy", "true");
	expect(page.getByRole("group", { name: "Archive main" }).elements()).toEqual(
		[],
	);
	release();
});

// Kills a filter written to the link with the spaces typed around it.
test("the filter's link holds the text without its spaces", async () => {
	const fake = api();
	const mounted = show(fake.api);
	dispose = mounted.dispose;
	await expect.element(rowOf("main")).toBeVisible();
	await userEvent.type(
		page.getByRole("searchbox", { name: "Filter branches" }),
		" simd ",
	);
	await expect
		.poll(() => mounted.history.get())
		.toBe("/hashbrown/branches?search=simd");
});

// Kills a first load that fails behind a skeleton forever.
test("a list that does not load says so, and Retry loads it", async () => {
	let failing = true;
	const fake = api({
		hold: async () => {
			if (failing) {
				throw new ApiError(503, "server", "{}");
			}
		},
	});
	dispose = show(fake.api).dispose;
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"Branches did not load: the Bencher API did not answer.",
		);
	failing = false;
	await page.getByRole("button", { name: "Retry" }).click();
	await expect.element(rowOf("main")).toBeVisible();
});

// Kills a next batch that fails without saying so or offering Retry.
test("a next batch that does not load says so, and Retry loads it", async () => {
	await page.viewport(1280, 720);
	let failing = true;
	const fake = pagingApi(numbered(60), {
		fail: (offset) => offset > 0 && failing,
	});
	dispose = show(fake.api).dispose;
	await expect.element(rowOf("branch-000")).toBeVisible();
	scrollToRow(25);
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"These rows did not load: the Bencher API did not answer.",
		);
	failing = false;
	await page.getByRole("button", { name: "Retry" }).click();
	await expect
		.element(branches().getByRole("link", { name: "branch-030" }))
		.toBeInTheDocument();
});
