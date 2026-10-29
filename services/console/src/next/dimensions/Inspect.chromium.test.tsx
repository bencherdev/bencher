import "../reports/reports.css";
import "./dimensions.css";
import type { QueryClient } from "@tanstack/solid-query";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonBenchmark,
	JsonBranch,
	JsonConsoleBranchRow,
	JsonMeasure,
	JsonTestbed,
	JsonThreshold,
	JsonVariant,
} from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { runFixture } from "../runs/testing";
import { type Sent, bootstrapOf, mount } from "../settings/testing";
import type { Dimension } from "./dimension";
import Inspect from "./Inspect";
import {
	FLOOR_WIDTHS,
	STRESS,
	expectFoldAt,
	expectNameStartsWithText,
	expectRowOpens,
	nameFloor,
} from "../ui/testing";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const BRANCH: JsonBranch = {
	uuid: "branch-uuid",
	project: "p",
	name: "412/merge",
	slug: "412-merge",
	head: {
		uuid: "head-uuid",
		start_point: {
			branch: "main-uuid",
			head: "main-head",
			version: { number: 7, hash: "4e02c1d0000000000000000000000000000000000" },
		},
		version: { number: 2, hash: "b71d0aa0000000000000000000000000000000000" },
		created: "2026-09-13T17:58:00Z",
	},
	created: "2026-09-12T10:00:00Z",
	modified: "2026-09-12T10:00:00Z",
};

const ROW: JsonConsoleBranchRow = {
	uuid: "branch-uuid",
	name: "412/merge",
	slug: "412-merge",
	start_point: "main",
	hash: "9c1f2e40000000000000000000000000000000000",
	created: Date.parse("2026-09-12T10:00:00Z"),
	last_report: Date.parse("2026-09-13T18:09:00Z"),
	thresholds: 2,
	held_thresholds: 1,
};

const threshold = (testbed: string): JsonThreshold =>
	({
		uuid: `threshold-${testbed}`,
		branch: { name: "412/merge" },
		testbed: { name: testbed },
		measure: { name: "Latency" },
		model: { test: "t_test", upper_boundary: 0.99, max_sample_size: 64 },
	}) as unknown as JsonThreshold;

const RUN = runFixture(0, {
	branch: {
		uuid: "branch-uuid",
		name: "412/merge",
		slug: "412-merge",
		head: "head-uuid",
	},
	reports: 2,
	testbeds: ["linux-x86-64", "macos-arm64"],
	testbed_count: 3,
	alerts: { total: 1, active: 1 },
});

const variant = (uuid: string, parameters: JsonVariant["parameters"]) =>
	({ uuid, benchmark: "b", parameters }) as unknown as JsonVariant;

type Answer = { data: unknown; total?: number } | ApiError;

/** A fake API: `answer` reads by path, and changes wait for `send`. */
const fakeApi = (
	answer: (url: URL) => Answer | Promise<Answer>,
	send: (sent: Sent) => Promise<unknown> = async () => undefined,
) => {
	const reads: URL[] = [];
	const sent: Sent[] = [];
	const api: Api = {
		get: async <T,>(path: string) => {
			const url = new URL(path, "http://api.test");
			reads.push(url);
			const answered = await answer(url);
			if (answered instanceof ApiError) {
				throw answered;
			}
			const headers = new Headers();
			if (answered.total !== undefined) {
				headers.set("X-Total-Count", String(answered.total));
			}
			return { data: answered.data as T, headers };
		},
		send: async <T,>(method: Sent["method"], path: string, body?: unknown) => {
			const change = { method, path, body };
			sent.push(change);
			return { data: (await send(change)) as T, headers: new Headers() };
		},
	};
	return { api, reads, sent };
};

/** A branch's API: a change it takes shows in its later reads. */
const branchApi = (
	archivedAtFirst: boolean,
	send: (sent: Sent) => Promise<unknown> = async () => undefined,
	{
		row = async () => {},
		detail = (): Answer | undefined => undefined,
	}: {
		/** What the console list read waits for before it answers. */
		row?: () => Promise<void>;
		/** An answer for the branch itself in place of the branch. */
		detail?: () => Answer | undefined;
	} = {},
) => {
	let archived = archivedAtFirst;
	return fakeApi(
		async (url) => {
			const path = url.pathname;
			const archivedRows = url.searchParams.get("archived") === "true";
			if (
				path === "/v0/projects/hashbrown/branches/412-merge" ||
				path === "/v0/projects/hashbrown/branches/branch-uuid"
			) {
				return (
					detail() ?? {
						data: archived
							? { ...BRANCH, archived: "2026-09-13T20:00:00Z" }
							: BRANCH,
					}
				);
			}
			if (path === "/v0/console/projects/hashbrown/branches") {
				await row();
				// The search matches a longer slug first, as the API's name order puts it.
				const matched =
					archivedRows === archived
						? [
								{ ...ROW, uuid: "other-uuid", slug: "412-merge-2" },
								archived ? { ...ROW, archived: 1 } : ROW,
							]
						: [];
				return {
					data: {
						branches: matched.slice(
							0,
							Number(url.searchParams.get("per_page")),
						),
					},
				};
			}
			if (path === "/v0/projects/hashbrown/thresholds") {
				return archivedRows === archived
					? {
							data: [threshold("linux-x86-64"), threshold("macos-arm64")],
							total: 2,
						}
					: { data: [], total: 0 };
			}
			if (path === "/v0/console/projects/hashbrown/runs") {
				// Runs leaves out an archived branch's runs.
				return { data: { total: 3, runs: archived ? [] : [RUN] } };
			}
			return new ApiError(404, "not_found", "{}");
		},
		async (change) => {
			const answer = await send(change);
			archived = (change.body as { archived: boolean }).archived;
			return answer;
		},
	);
};

/** The branches list as the cache holds it: one batch with its totals. */
const LIST_KEY = ["console", "branches", "hashbrown", "list", "", 26, 1];
const listBatch = (archived?: number) => ({
	rows: [archived === undefined ? ROW : { ...ROW, archived }],
	total: 1,
	active: 3,
	archived: 1,
	batch: { ordinal: 1, perPage: 26 },
});
const listTotals = (client: QueryClient) => {
	const batch = client.getQueryData<{ active: number; archived: number }>(
		LIST_KEY,
	);
	return { active: batch?.active, archived: batch?.archived };
};

const show = async (
	api: Api,
	dimension: Dimension,
	entry: string,
	edit = true,
	cached: [unknown[], unknown][] = [],
) => {
	await page.viewport(1280, 720);
	return mount(
		() => <Inspect dimension={dimension} entry={entry} edit={edit} />,
		{
			api,
			path: `/hashbrown/${dimension}/${entry}`,
			bootstrap: bootstrapOf({ edit }),
			cached,
		},
	);
};

// Kills a page that drops its start point's name, its head, or the thresholds
// on it, threshold links named only by their testbed, a threshold's model cut
// to its test, a row taken from the first match alone, a recent run that opens
// anything but its run or reads another run's head, testbeds, or alerts, a
// testbed count whose hover leaves out the rest, and an All runs link that
// keeps the default window.
test("a branch's page shows its start point, head, recent runs, and thresholds", async () => {
	const { api, reads } = branchApi(false);
	dispose = (await show(api, "branches", "412-merge")).dispose;
	await expect
		.element(page.getByRole("heading", { level: 1, name: "412/merge" }))
		.toBeVisible();
	await expect.element(page.getByText(/^from main/)).toBeInTheDocument();
	const head = page.getByRole("region", { name: "Head" });
	await expect.element(head).toHaveTextContent(/version\s*9c1f2e4/);
	await expect.element(head).toHaveTextContent(/main at 4e02c1d/);
	await expect
		.element(head.getByRole("link", { name: "main" }))
		.toHaveAttribute("href", "/console/projects/hashbrown/branches/main-uuid");
	const thresholds = page.getByRole("table", { name: "Thresholds" });
	const link = thresholds.getByRole("link", {
		name: "macos-arm64, threshold for Latency on 412/merge, macos-arm64",
	});
	await expect
		.element(link)
		.toHaveAttribute(
			"href",
			"/console/projects/hashbrown/thresholds/threshold-macos-arm64",
		);
	expectNameStartsWithText(link.element());
	await expect
		.element(thresholds.getByRole("row").nth(1))
		.toHaveTextContent("t_test · upper_boundary 0.99 · max_sample_size 64");
	const recent = page.getByRole("table", { name: "Recent runs" });
	const run = recent.getByRole("link", {
		name: /, run 9c1f2e4 on 412\/merge, 2 reports$/,
	});
	await expect
		.element(run)
		.toHaveAttribute("href", `/console/projects/hashbrown/runs/${RUN.uuid}`);
	expectNameStartsWithText(run.element());
	await expect
		.element(recent.getByRole("row").nth(1))
		.toHaveTextContent(/9c1f2e4\s*2\s*linux-x86-64\s*\+2\s*1 active/);
	await expect
		.element(recent.getByText("+2"))
		.toHaveAttribute("title", "linux-x86-64, macos-arm64, and 1 more");
	expect(
		reads
			.filter(({ pathname }) => pathname.endsWith("/runs"))
			.map(({ searchParams }) => searchParams.get("branch")),
	).toEqual(["412-merge"]);
	await expect
		.element(page.getByRole("link", { name: "All runs on 412/merge" }))
		.toHaveAttribute(
			"href",
			"/console/projects/hashbrown/runs?branch=412-merge&window=all",
		);
});

// Kills a branch's recent runs or thresholds that keep their columns where a
// testbed's name falls under 16 characters, and one that scrolls sideways in
// its own track instead.
test("a branch's recent runs and thresholds hold 16 characters of name, or fold", async () => {
	const { api } = fakeApi((url) => {
		const path = url.pathname;
		if (path === "/v0/projects/hashbrown/branches/412-merge") {
			return { data: BRANCH };
		}
		if (path === "/v0/console/projects/hashbrown/branches") {
			return { data: { branches: [ROW] } };
		}
		if (path === "/v0/projects/hashbrown/thresholds") {
			return { data: [threshold(STRESS.name)], total: 1 };
		}
		if (path === "/v0/console/projects/hashbrown/runs") {
			return {
				data: { total: 1, runs: [{ ...RUN, testbeds: [STRESS.name] }] },
			};
		}
		return new ApiError(404, "not_found", "{}");
	});
	const held: Record<string, unknown> = {};
	for (const width of FLOOR_WIDTHS) {
		await page.viewport(width, 720);
		dispose = mount(
			() => <Inspect dimension="branches" entry="412-merge" edit={true} />,
			{ api, path: "/hashbrown/branches/412-merge", bootstrap: bootstrapOf() },
		).dispose;
		for (const name of ["Recent runs", "Thresholds"]) {
			const table = page.getByRole("table", { name });
			await expect.element(table).toBeVisible();
			held[`${name} ${width}`] = nameFloor(table.element(), "Testbed");
		}
		dispose();
		dispose = undefined;
	}
	const folded = { name: "folded", sideways: false };
	const fine = { name: "held", sideways: false };
	expect(held).toEqual({
		"Recent runs 320": folded,
		"Recent runs 768": fine,
		"Recent runs 1024": fine,
		"Recent runs 1440": fine,
		"Thresholds 320": folded,
		"Thresholds 768": fine,
		"Thresholds 1024": fine,
		"Thresholds 1440": fine,
	});
});

// Kills a card's table that folds a pixel late or early.
test("a branch's recent runs and thresholds, and a benchmark's variants, fold at their own widths", async () => {
	const { api } = fakeApi((url) => {
		const path = url.pathname;
		if (path === "/v0/projects/hashbrown/branches/412-merge") {
			return { data: BRANCH };
		}
		if (path === "/v0/console/projects/hashbrown/branches") {
			return { data: { branches: [ROW] } };
		}
		if (path === "/v0/projects/hashbrown/thresholds") {
			return { data: [threshold(STRESS.name)], total: 1 };
		}
		if (path === "/v0/console/projects/hashbrown/runs") {
			return {
				data: { total: 1, runs: [{ ...RUN, testbeds: [STRESS.name] }] },
			};
		}
		return new ApiError(404, "not_found", "{}");
	});
	dispose = (await show(api, "branches", "412-merge")).dispose;
	for (const [name, fold] of [
		["Recent runs", 533],
		["Thresholds", 615],
	] as const) {
		const table = page.getByRole("table", { name });
		await expect.element(table).toBeVisible();
		await document.fonts.ready;
		expectFoldAt(table.element(), "Testbed", fold);
	}
	dispose();
	dispose = (await show(benchmarkApi().api, "benchmarks", "blake3")).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	expectFoldAt(variantsTable().element(), "Variant", 215);
});

const GIB = 1024 ** 3;

/** One page of each dimension, each carrying a threshold, a run, or variants. */
const layoutApi = () =>
	fakeApi((url) => {
		const path = url.pathname;
		const details: Record<string, unknown> = {
			"/v0/projects/hashbrown/branches/412-merge": BRANCH,
			"/v0/projects/hashbrown/testbeds/ubuntu-latest": {
				uuid: "testbed-uuid",
				name: "ubuntu-latest",
				slug: "ubuntu-latest",
				spec: {
					uuid: "spec-uuid",
					name: "Standard x86",
					slug: "standard-x86",
					os: "linux",
					architecture: "x86_64",
					cpu: 8,
					memory: 16 * GIB,
					disk: 75 * GIB,
					network: false,
				},
				created: "2026-09-12T10:00:00Z",
			} as JsonTestbed,
			"/v0/projects/hashbrown/measures/latency": {
				uuid: "measure-uuid",
				name: "Latency",
				slug: "latency",
				units: "nanoseconds (ns)",
				created: "2026-09-12T10:00:00Z",
			} as JsonMeasure,
			"/v0/projects/hashbrown/benchmarks/blake3": {
				uuid: "blake3-uuid",
				name: "blake3",
				slug: "blake3",
				created: "2026-09-12T10:00:00Z",
			} as JsonBenchmark,
		};
		if (path in details) {
			return { data: details[path] };
		}
		if (path === "/v0/projects/hashbrown/thresholds") {
			return { data: [threshold("linux-x86-64")], total: 1 };
		}
		if (path === "/v0/console/projects/hashbrown/runs") {
			return { data: { total: 1, runs: [RUN] } };
		}
		if (path.endsWith("/variants")) {
			return { data: VARIANTS, total: VARIANTS.length };
		}
		return { data: {} };
	});

// Kills a page whose sections leave the ruled order or sit in two columns
// read out of it, the count boxes left at the top, and Open in Explore kept.
test.each([
	["branches", "412-merge", ["Record", "Head", "Recent runs", "Thresholds"]],
	[
		"testbeds",
		"ubuntu-latest",
		["Record", "Spec", "Recent runs", "Thresholds"],
	],
	["benchmarks", "blake3", ["Record", "Parameters in use", "Variants"]],
	["measures", "latency", ["Record", "Units", "Thresholds"]],
] as const)(
	"a page of %s stacks its sections in order",
	async (dimension, entry, sections) => {
		const { api } = layoutApi();
		dispose = (await show(api, dimension, entry)).dispose;
		await expect.element(page.getByRole("heading", { level: 1 })).toBeVisible();
		const headings = page.getByRole("heading", { level: 2 }).elements();
		expect(headings.map((heading) => heading.textContent)).toEqual(sections);
		const boxes = headings.map((heading) =>
			heading.closest("section")?.getBoundingClientRect(),
		);
		expect(new Set(boxes.map((box) => box?.left)).size).toBe(1);
		const tops = boxes.map((box) => box?.top ?? 0);
		expect(tops).toEqual([...tops].sort((a, b) => a - b));
		expect(page.getByRole("list").elements()).toEqual([]);
		expect(page.getByRole("link", { name: /Explore/ }).elements()).toEqual([]);
		await expect
			.element(page.getByRole("region", { name: "Record" }))
			.toHaveTextContent(new RegExp(`^Record\\s*slug\\s*${entry}\\s*uuid`));
	},
);

// Kills a spec that drops its name or slug or moves them below its machine,
// a testbed's runs read by branch rather than by testbed, a run row that opens
// anything but its run, and an All runs link that filters by branch.
test("a testbed's page leads its spec with its name and slug and lists its recent runs", async () => {
	const { api, reads } = layoutApi();
	dispose = (await show(api, "testbeds", "ubuntu-latest")).dispose;
	await expect
		.element(page.getByRole("region", { name: "Spec" }))
		.toHaveTextContent(
			/^Spec\s*name\s*Standard x86\s*slug\s*standard-x86\s*os\s*linux/,
		);
	expect(
		reads
			.filter(({ pathname }) => pathname.endsWith("/runs"))
			.map(({ searchParams }) => [
				searchParams.get("testbed"),
				searchParams.get("branch"),
			]),
	).toEqual([["ubuntu-latest", null]]);
	const recent = page.getByRole("table", { name: "Recent runs" });
	await expect
		.element(recent.getByRole("row").nth(1))
		.toHaveTextContent(/412\/merge\s*9c1f2e4\s*2\s*1 active/);
	await expect
		.element(
			recent.getByRole("link", {
				name: /, run 9c1f2e4 on 412\/merge, 2 reports$/,
			}),
		)
		.toHaveAttribute("href", `/console/projects/hashbrown/runs/${RUN.uuid}`);
	await expect
		.element(page.getByRole("link", { name: "All runs on ubuntu-latest" }))
		.toHaveAttribute(
			"href",
			"/console/projects/hashbrown/runs?testbed=ubuntu-latest&window=all",
		);
});

// Kills a recent run or threshold row only its first cell opens, and one
// whose link covers the rows around it, wide or folded on a phone.
test("a branch's recent runs and thresholds open from anywhere on their rows", async () => {
	for (const [width, height] of [
		[1280, 720],
		[390, 844],
	] as const) {
		const { api } = branchApi(false);
		dispose = (await show(api, "branches", "412-merge")).dispose;
		await page.viewport(width, height);
		const rows = () =>
			[
				...page
					.getByRole("table", { name: "Recent runs" })
					.getByRole("row")
					.elements(),
				...page
					.getByRole("table", { name: "Thresholds" })
					.getByRole("row")
					.elements(),
			].filter((row) => row.querySelector("a"));
		await expect.poll(() => rows().length).toBe(3);
		for (const row of rows()) {
			row.scrollIntoView({ block: "center" });
			expectRowOpens(
				row,
				row.querySelector("a")?.getAttribute("href") ?? "no link",
			);
		}
		dispose?.();
		dispose = undefined;
	}
});

// Kills an archive that waits for the API before the page says so, an Undo
// that is not offered, and a refused archive left showing as archived.
test("archiving from the page shows it at once, and a refusal puts it back", async () => {
	let refuse: (error: unknown) => void = () => {};
	const { api, sent } = branchApi(
		false,
		() =>
			new Promise((_, reject) => {
				refuse = reject;
			}),
	);
	dispose = (await show(api, "branches", "412-merge")).dispose;
	await page.getByRole("button", { name: "Archive" }).click();
	const impact = page.getByRole("group", { name: "Archive 412/merge" });
	await expect
		.element(impact)
		.toHaveTextContent("Hides its lines and archives 2 thresholds");
	await impact.getByRole("button", { name: "Archive 412/merge" }).click();

	await expect
		.element(page.getByText("Archived", { exact: true }))
		.toBeVisible();
	await expect
		.element(page.getByRole("status"))
		.toHaveTextContent("Archived 412/merge and 2 thresholds");
	await expect
		.element(page.getByRole("button", { name: "Unarchive" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("status").getByRole("button", { name: "Undo" }))
		.toBeVisible();
	// The confirm left with the impact line; the focus moves to Undo.
	await expect.poll(() => document.activeElement?.textContent).toBe("Undo");
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/projects/hashbrown/branches/branch-uuid",
			body: { archived: true },
		},
	]);

	refuse(new ApiError(503, "server", '{"message":"Busy"}'));
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not archive 412/merge: Busy");
	await expect
		.element(page.getByRole("button", { name: "Archive" }))
		.toBeVisible();
	expect(page.getByText("Archived", { exact: true }).elements()).toEqual([]);
});

// Kills an archived branch's page that lists its active thresholds (none),
// recent runs left loading when Runs lists none of an archived branch's, an
// All runs link to a list that would show none of them, and an Unarchive that
// claims every threshold back.
test("an archived branch reads its archived thresholds and says which come back", async () => {
	const { api, reads } = branchApi(true);
	dispose = (await show(api, "branches", "412-merge")).dispose;
	await expect
		.element(
			page
				.getByRole("table", { name: "Thresholds" })
				.getByRole("link", { name: "linux-x86-64" }),
		)
		.toBeVisible();
	expect(
		reads
			.filter(({ pathname }) => pathname.endsWith("/thresholds"))
			.map(({ searchParams }) => searchParams.get("archived")),
	).toContain("true");
	await expect
		.element(page.getByRole("region", { name: "Recent runs" }))
		.toHaveTextContent(/No runs$/);
	expect(page.getByRole("link", { name: /^All runs/ }).elements()).toEqual([]);

	await page.getByRole("button", { name: "Unarchive" }).click();
	await expect
		.element(page.getByRole("status"))
		.toHaveTextContent(
			"Unarchived 412/merge and 2 thresholds · 1 threshold still archived",
		);
});

// Kills a viewer offered Archive.
test("a reader who cannot edit sees the read only line instead of Archive", async () => {
	const { api } = branchApi(false);
	dispose = (await show(api, "branches", "412-merge", false)).dispose;
	await expect
		.element(page.getByText("Read only", { exact: true }))
		.toBeVisible();
	expect(page.getByRole("button", { name: /Archive/ }).elements()).toEqual([]);
});

// Kills a missing dimension drawn as a page that never loads.
test("a branch the project does not have says so", async () => {
	const { api } = branchApi(false);
	dispose = (await show(api, "branches", "nope")).dispose;
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Branch not found" }))
		.toBeVisible();
});

// Kills a variant archive that waits for the API, and the never reported
// empty variant listed beside the reported ones.
test("a benchmark's variant archives at once from its row", async () => {
	const { api, sent } = fakeApi(
		(url) => {
			if (url.pathname === "/v0/projects/hashbrown/benchmarks/blake3") {
				return {
					data: {
						uuid: "blake3-uuid",
						name: "blake3",
						slug: "blake3",
						created: "2026-09-12T10:00:00Z",
					} as JsonBenchmark,
				};
			}
			if (url.pathname.endsWith("/variants")) {
				return url.searchParams.get("archived") === "true"
					? { data: [], total: 0 }
					: {
							data: [
								variant("empty", {}),
								variant("one", { threads: 1, input_bytes: 1024 }),
								variant("four", { threads: 4, input_bytes: 1024 }),
							],
							total: 3,
						};
			}
			return { data: { benchmarks: [] } };
		},
		() => new Promise(() => {}),
	);
	dispose = (await show(api, "benchmarks", "blake3")).dispose;
	const variants = page.getByRole("table", { name: "Active variants" });
	await expect.element(variants.getByText("threads=4")).toBeVisible();
	expect(variants.getByRole("row").elements()).toHaveLength(3);
	await expect
		.element(page.getByRole("region", { name: "Parameters in use" }))
		.toHaveTextContent(/input_bytes\s*10242 variants/);

	await variants.getByRole("button", { name: "Archive threads=4" }).click();
	await page
		.getByRole("group", { name: "Archive threads=4" })
		.getByRole("button", { name: "Archive variant" })
		.click();
	await expect
		.element(variants.getByRole("row").filter({ hasText: "threads=4" }))
		.toHaveTextContent(/Archived just now/);
	await expect
		.element(page.getByRole("radio", { name: "Archived 1" }))
		.toBeInTheDocument();
	// An archived variant's values are no longer in use.
	await expect
		.element(page.getByRole("region", { name: "Parameters in use" }))
		.toHaveTextContent(/input_bytes\s*10241 variant/);
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/projects/hashbrown/benchmarks/blake3/variants/four",
			body: { archived: true },
		},
	]);
});

/** Archive the page's dimension through its impact line. */
const archivePage = async (name: string) => {
	await page.getByRole("button", { name: "Archive", exact: true }).click();
	await page
		.getByRole("group", { name: `Archive ${name}` })
		.getByRole("button", { name: `Archive ${name}` })
		.click();
};

const held = () => {
	const pending: { resolve: () => void; reject: (error: unknown) => void }[] =
		[];
	const send = () =>
		new Promise<unknown>((resolve, reject) => {
			pending.push({ resolve: () => resolve(undefined), reject });
		});
	return { pending, send };
};

// Kills a branch page by UUID that loses its row, which carries its start
// point's name and its newest hash.
test("a branch's page by its UUID shows what its row carries", async () => {
	const { api } = branchApi(false);
	dispose = (await show(api, "branches", "branch-uuid")).dispose;
	await expect
		.element(page.getByRole("heading", { level: 1, name: "412/merge" }))
		.toBeVisible();
	await expect.element(page.getByText(/^from main/)).toBeInTheDocument();
	await expect
		.element(page.getByRole("region", { name: "Head" }))
		.toHaveTextContent(/version\s*9c1f2e4/);
});

// Kills a page that paints before its row answers, which moves the subtitle
// and the head once it does.
test("the page paints once its row answers too", async () => {
	let release = () => {};
	const waiting = new Promise<void>((resolve) => {
		release = resolve;
	});
	const { api, reads } = branchApi(false, undefined, { row: () => waiting });
	dispose = (await show(api, "branches", "412-merge")).dispose;
	await expect
		.poll(() => reads.some(({ pathname }) => pathname.endsWith("/runs")))
		.toBe(true);
	await new Promise((resolve) => setTimeout(resolve, 100));
	expect(page.getByRole("heading", { level: 1 }).elements()).toEqual([]);
	release();
	await expect
		.element(page.getByRole("heading", { level: 1, name: "412/merge" }))
		.toBeVisible();
	await expect.element(page.getByText(/^from main/)).toBeInTheDocument();
});

// Kills a page that reads any failure as a missing branch.
test("a branch that does not load says so, and Retry loads it", async () => {
	let failing = true;
	const { api } = branchApi(false, undefined, {
		detail: () => (failing ? new ApiError(503, "server", "{}") : undefined),
	});
	dispose = (await show(api, "branches", "412-merge")).dispose;
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("This branch did not load");
	expect(page.getByRole("heading", { name: /^No branch/ }).elements()).toEqual(
		[],
	);
	failing = false;
	await page.getByRole("button", { name: "Retry" }).click();
	await expect
		.element(page.getByRole("heading", { level: 1, name: "412/merge" }))
		.toBeVisible();
});

// Kills an Undo on the page sent before the archive it undoes, and a refusal
// that rolls the page and the cached list back over that queued Undo, which
// left the list at "Active 4" and "Archived 0".
test("on the page, an Undo before the archive lands is sent after it, and a refused archive leaves the Undo standing", async () => {
	const { pending, send } = held();
	const { api, sent } = branchApi(false, send);
	const mounted = await show(api, "branches", "412-merge", true, [
		[LIST_KEY, listBatch()],
	]);
	dispose = mounted.dispose;
	await archivePage("412/merge");
	await expect
		.poll(() => listTotals(mounted.client))
		.toEqual({ active: 2, archived: 2 });
	await page.getByRole("status").getByRole("button", { name: "Undo" }).click();
	await expect
		.element(page.getByRole("button", { name: "Archive", exact: true }))
		.toBeVisible();
	await new Promise((resolve) => setTimeout(resolve, 50));
	expect(sent.length).toBe(1);

	pending[0]?.reject(new ApiError(409, "client", '{"message":"Busy"}'));
	await expect
		.poll(() => sent.map(({ body }) => body))
		.toEqual([{ archived: true }, { archived: false }]);
	pending[1]?.resolve();
	await new Promise((resolve) => setTimeout(resolve, 50));
	expect(listTotals(mounted.client)).toEqual({ active: 3, archived: 1 });
	await expect
		.element(page.getByRole("button", { name: "Archive", exact: true }))
		.toBeVisible();
	expect(page.getByText("Archived", { exact: true }).elements()).toEqual([]);
});

// Kills a refusal on the page that leaves the cached list showing the change,
// and one that rolls back to before the archive rather than to what the API
// last confirmed.
test("a refused Undo on the page after the archive landed shows it archived again, in the list too", async () => {
	let calls = 0;
	const { api } = branchApi(false, async () => {
		calls += 1;
		if (calls === 2) {
			throw new ApiError(409, "client", '{"message":"Busy"}');
		}
	});
	const mounted = await show(api, "branches", "412-merge", true, [
		[LIST_KEY, listBatch()],
	]);
	dispose = mounted.dispose;
	await archivePage("412/merge");
	await expect.poll(() => calls).toBe(1);
	await page.getByRole("status").getByRole("button", { name: "Undo" }).click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not unarchive 412/merge: Busy");
	await expect
		.element(page.getByText("Archived", { exact: true }))
		.toBeVisible();
	expect(listTotals(mounted.client)).toEqual({ active: 2, archived: 2 });
	expect(
		mounted.client.getQueryData<{ rows: { archived?: number }[] }>(LIST_KEY)
			?.rows[0]?.archived,
	).toBeTypeOf("number");
});

// Kills a refusal on the page that rolls back while a later change is queued,
// which left the cached list counting the branch active once that later
// archive landed.
test("a refused archive on the page leaves it to the later changes queued after it", async () => {
	const { pending, send } = held();
	const { api, sent } = branchApi(false, send);
	const mounted = await show(api, "branches", "412-merge", true, [
		[LIST_KEY, listBatch()],
	]);
	dispose = mounted.dispose;
	await archivePage("412/merge");
	await page.getByRole("status").getByRole("button", { name: "Undo" }).click();
	await archivePage("412/merge");
	await expect.poll(() => sent.length).toBe(1);

	pending[0]?.reject(new ApiError(409, "client", '{"message":"Busy"}'));
	await expect.poll(() => sent.length).toBe(2);
	pending[1]?.resolve();
	await expect.poll(() => sent.length).toBe(3);
	pending[2]?.resolve();
	await expect
		.element(page.getByText("Archived", { exact: true }))
		.toBeVisible();
	await expect
		.poll(() => listTotals(mounted.client))
		.toEqual({ active: 2, archived: 2 });
});

// Kills a page that moves the cached list's totals by what the list shows of
// the branch when the list does not hold it, so an Undo left them archived.
test("archiving from the page and undoing it moves the list's totals there and back, whatever batch holds the branch", async () => {
	const { api, sent } = branchApi(false);
	const mounted = await show(api, "branches", "412-merge", true, [
		[LIST_KEY, { ...listBatch(), rows: [{ ...ROW, uuid: "other-uuid" }] }],
	]);
	dispose = mounted.dispose;
	await archivePage("412/merge");
	await expect.poll(() => sent.length).toBe(1);
	await expect
		.poll(() => listTotals(mounted.client))
		.toEqual({ active: 2, archived: 2 });
	await page.getByRole("status").getByRole("button", { name: "Undo" }).click();
	await expect.poll(() => sent.length).toBe(2);
	await expect
		.poll(() => listTotals(mounted.client))
		.toEqual({ active: 3, archived: 1 });
});

// Kills a refused archive on the page that leaves the cached list archived.
test("a refused archive on the page puts the cached list back too", async () => {
	const { api } = branchApi(false, async () => {
		throw new ApiError(409, "client", '{"message":"Busy"}');
	});
	const mounted = await show(api, "branches", "412-merge", true, [
		[LIST_KEY, listBatch()],
	]);
	dispose = mounted.dispose;
	await archivePage("412/merge");
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not archive 412/merge: Busy");
	expect(listTotals(mounted.client)).toEqual({ active: 3, archived: 1 });
	expect(
		mounted.client.getQueryData<{ rows: { archived?: number }[] }>(LIST_KEY)
			?.rows[0]?.archived,
	).toBeUndefined();
});

// Kills a landed change on the page that leaves the rest of the project
// stale, such as the shell's alert count.
test("a landed archive on the page marks the rest of the project stale", async () => {
	const { api, sent } = branchApi(false);
	const mounted = await show(api, "branches", "412-merge");
	dispose = mounted.dispose;
	await archivePage("412/merge");
	await expect.poll(() => sent.length).toBe(1);
	await expect
		.poll(
			() =>
				mounted.client.getQueryState(["console", "project", "hashbrown"])
					?.isInvalidated,
		)
		.toBe(true);
});

// Kills a Cancel or an Escape that leaves the focus on a control that is gone.
test("Cancel and Escape on the page's impact line give the focus back to Archive", async () => {
	const { api } = branchApi(false);
	dispose = (await show(api, "branches", "412-merge")).dispose;
	const archive = page.getByRole("button", { name: "Archive", exact: true });
	for (const close of [
		() =>
			page
				.getByRole("group", { name: "Archive 412/merge" })
				.getByRole("button", { name: "Cancel" })
				.click(),
		() => userEvent.keyboard("{Escape}"),
	]) {
		await archive.click();
		await expect
			.poll(() => document.activeElement?.textContent)
			.toBe("Archive 412/merge");
		await close();
		await expect
			.poll(() =>
				page.getByRole("group", { name: "Archive 412/merge" }).elements(),
			)
			.toEqual([]);
		await expect.poll(() => document.activeElement).toBe(archive.element());
	}
});

const VARIANTS = [
	variant("empty", {}),
	variant("one", { threads: 1 }),
	variant("four", { threads: 4 }),
];

/**
 * A benchmark with the empty variant it was born with and two that report
 * parameters; a change it takes shows in its later reads.
 */
const benchmarkApi = (
	send: (sent: Sent) => Promise<unknown> = async () => {},
) => {
	const archived = new Set<string>();
	return fakeApi(
		(url) => {
			if (url.pathname === "/v0/projects/hashbrown/benchmarks/blake3") {
				return {
					data: {
						uuid: "blake3-uuid",
						name: "blake3",
						slug: "blake3",
						created: "2026-09-12T10:00:00Z",
					} as JsonBenchmark,
				};
			}
			if (url.pathname.endsWith("/variants")) {
				const listed = VARIANTS.filter(
					({ uuid }) =>
						archived.has(uuid) ===
						(url.searchParams.get("archived") === "true"),
				).map((each) =>
					archived.has(each.uuid)
						? { ...each, archived: "2026-09-14T00:00:00Z" }
						: each,
				);
				return { data: listed, total: listed.length };
			}
			return { data: { benchmarks: [] } };
		},
		async (change) => {
			const answer = await send(change);
			const uuid = change.path.split("/").at(-1) ?? "";
			if ((change.body as { archived: boolean }).archived) {
				archived.add(uuid);
			} else {
				archived.delete(uuid);
			}
			return answer;
		},
	);
};

const variantsTable = () =>
	page.getByRole("table", { name: "Active variants" });
const variantRow = (tag: string) =>
	variantsTable().getByRole("row").filter({ hasText: tag });

const archiveVariant = async (tag: string) => {
	await variantsTable()
		.getByRole("button", { name: `Archive ${tag}` })
		.click();
	await page
		.getByRole("group", { name: `Archive ${tag}` })
		.getByRole("button", { name: "Archive variant" })
		.click();
};

/** The variant toggle's counts, as the page reads them. */
const variantCounts = () =>
	page
		.getByRole("radio")
		.elements()
		.map((radio) => radio.closest("label")?.textContent?.replace(/\s+/g, " "));

// Kills counts that include the empty variant the table hides, and counts
// that keep a variant archived on screen in Active.
test("a benchmark's counts are the variants its table shows, each in its shown state", async () => {
	const { api } = benchmarkApi();
	dispose = (await show(api, "benchmarks", "blake3")).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	expect(variantCounts()).toEqual(["Active 2", "Archived 0"]);
	await archiveVariant("threads=4");
	await expect
		.element(variantRow("threads=4"))
		.toHaveTextContent(/Archived just now/);
	expect(variantCounts()).toEqual(["Active 1", "Archived 1"]);
});

// Kills a variant change that refetches the list on screen once it lands,
// which drops the dimmed row and its Undo, and one that leaves the other
// status's list stale.
test("a landed variant archive keeps its row dimmed with Undo and rereads the archived list", async () => {
	const { api, reads, sent } = benchmarkApi();
	dispose = (await show(api, "benchmarks", "blake3")).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	const listed = (archived: string) =>
		reads.filter(
			({ pathname, searchParams }) =>
				pathname.endsWith("/variants") &&
				searchParams.get("archived") === archived,
		).length;
	const before = { active: listed("false"), archived: listed("true") };
	await archiveVariant("threads=4");
	await expect.poll(() => sent.length).toBe(1);
	await expect.poll(() => listed("true")).toBe(before.archived + 1);
	await new Promise((resolve) => setTimeout(resolve, 100));
	expect(listed("false")).toBe(before.active);
	await expect
		.element(
			variantRow("threads=4").getByRole("button", {
				name: "Undo, threads=4",
			}),
		)
		.toBeVisible();
});

// Kills a variant Undo sent before the archive it undoes, and a refusal that
// rolls the variant back over that queued Undo, which read "Active 4" and
// "Archived -1".
test("a variant's Undo before its archive lands is sent after it, and a refused archive leaves the Undo standing", async () => {
	const { pending, send } = held();
	const { api, sent } = benchmarkApi(send);
	dispose = (await show(api, "benchmarks", "blake3")).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	await archiveVariant("threads=4");
	await variantsTable()
		.getByRole("button", { name: "Undo, threads=4" })
		.click();
	await new Promise((resolve) => setTimeout(resolve, 50));
	expect(sent.length).toBe(1);

	pending[0]?.reject(new ApiError(409, "client", '{"message":"Busy"}'));
	await expect
		.poll(() => sent.map(({ body }) => body))
		.toEqual([{ archived: true }, { archived: false }]);
	pending[1]?.resolve();
	await new Promise((resolve) => setTimeout(resolve, 50));
	expect(variantCounts()).toEqual(["Active 2", "Archived 0"]);
	await expect
		.element(
			variantRow("threads=4").getByRole("button", {
				name: "Archive threads=4",
			}),
		)
		.toBeVisible();
});

// Kills a variant refusal that rolls back while a later change on it is
// queued, which left the variant active once that later archive landed.
test("a refused variant archive leaves the variant to the later changes queued on it", async () => {
	const { pending, send } = held();
	const { api, sent } = benchmarkApi(send);
	dispose = (await show(api, "benchmarks", "blake3")).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	await archiveVariant("threads=4");
	await variantsTable()
		.getByRole("button", { name: "Undo, threads=4" })
		.click();
	await archiveVariant("threads=4");
	await expect.poll(() => sent.length).toBe(1);

	pending[0]?.reject(new ApiError(409, "client", '{"message":"Busy"}'));
	await expect.poll(() => sent.length).toBe(2);
	pending[1]?.resolve();
	await expect.poll(() => sent.length).toBe(3);
	pending[2]?.resolve();
	await expect
		.element(variantRow("threads=4"))
		.toHaveTextContent(/Archived just now/);
	await expect.poll(variantCounts).toEqual(["Active 1", "Archived 1"]);
});

// Kills a refused variant Undo that puts the variant back as it was before
// the archive rather than as the API last confirmed it.
test("a refused variant Undo after the archive landed shows it archived again", async () => {
	let calls = 0;
	const { api } = benchmarkApi(async () => {
		calls += 1;
		if (calls === 2) {
			throw new ApiError(409, "client", '{"message":"Busy"}');
		}
	});
	dispose = (await show(api, "benchmarks", "blake3")).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	await archiveVariant("threads=4");
	await expect.poll(() => calls).toBe(1);
	await variantsTable()
		.getByRole("button", { name: "Undo, threads=4" })
		.click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not unarchive the variant: Busy");
	await expect
		.element(variantRow("threads=4"))
		.toHaveTextContent(/Archived just now/);
	expect(variantCounts()).toEqual(["Active 1", "Archived 1"]);
});

// Kills a refused variant archive left showing as archived.
test("a refused variant archive puts the variant and its counts back", async () => {
	const { api } = benchmarkApi(async () => {
		throw new ApiError(409, "client", '{"message":"Busy"}');
	});
	dispose = (await show(api, "benchmarks", "blake3")).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	await archiveVariant("threads=4");
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Bencher did not archive the variant: Busy");
	await expect
		.element(
			variantRow("threads=4").getByRole("button", {
				name: "Archive threads=4",
			}),
		)
		.toBeVisible();
	expect(variantCounts()).toEqual(["Active 2", "Archived 0"]);
});

// Kills a variant confirm, Undo, Cancel, or Escape that drops the focus with
// the control it replaces.
test("the focus follows a variant through its impact line, Undo, Cancel, and Escape", async () => {
	const { api } = benchmarkApi();
	dispose = (await show(api, "benchmarks", "blake3")).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	const focused = () =>
		document.activeElement?.getAttribute("aria-label") ??
		document.activeElement?.textContent;

	await archiveVariant("threads=4");
	await expect.poll(focused).toBe("Undo, threads=4");
	await variantsTable()
		.getByRole("button", { name: "Undo, threads=4" })
		.click();
	await expect.poll(focused).toBe("Archive threads=4");

	const archive = variantsTable().getByRole("button", {
		name: "Archive threads=1",
	});
	for (const close of [
		() =>
			page
				.getByRole("group", { name: "Archive threads=1" })
				.getByRole("button", { name: "Cancel" })
				.click(),
		() => userEvent.keyboard("{Escape}"),
	]) {
		await archive.click();
		await expect.poll(focused).toBe("Archive variant");
		await close();
		await expect.poll(() => document.activeElement).toBe(archive.element());
	}
});

// Kills Archive on a variant drawn for a reader the API would refuse.
test("a reader who cannot edit sees a benchmark's variants with no control", async () => {
	const { api } = benchmarkApi();
	dispose = (await show(api, "benchmarks", "blake3", false)).dispose;
	await expect.element(variantsTable().getByText("threads=4")).toBeVisible();
	expect(page.getByRole("button", { name: /Archive|Undo/ }).elements()).toEqual(
		[],
	);
});
