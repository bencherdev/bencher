import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./explore.css";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonConsoleReport, JsonReport } from "../../types/bencher";
import type { Api } from "../api";
import { type ExploreQuery, blankQuery } from "../query/query";
import { mount } from "../settings/testing";
import { type Dimension, nameQuery } from "./queries";
import Start from "./Start";
import { reportBenchmarksQuery } from "./start";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
});

const api: Api = {
	get: async <T,>() => ({ data: [] as T, headers: new Headers() }),
	send: async () => {
		throw new Error("The start lists send no changes");
	},
};

// Kills starting points out of the tabs' order, or drawn in an order that
// their reading and keyboard order does not follow.
for (const [width, edge] of [
	[1280, "left"],
	[390, "top"],
] as const) {
	test(`the starting points run Pinned plots, Latest reports, Alerting now at ${width} px`, async () => {
		await page.viewport(width, 844);
		dispose = mount(() => <Start onQuery={() => {}} />, { api }).dispose;
		const lists = ["Pinned plots", "Latest reports", "Alerting now"].map(
			(name) => page.getByRole("region", { name }).element(),
		);
		for (const [at, list] of lists.entries()) {
			const next = lists[at + 1];
			if (next) {
				expect(
					list.compareDocumentPosition(next) & Node.DOCUMENT_POSITION_FOLLOWING,
				).toBeTruthy();
				expect(next.getBoundingClientRect()[edge]).toBeGreaterThan(
					list.getBoundingClientRect()[edge],
				);
			}
		}
	});
}

// Kills a link to a whole list too small to tap on a phone.
test("on a phone each list's link to all of it is a 44 px target", async () => {
	await page.viewport(390, 844);
	dispose = mount(() => <Start onQuery={() => {}} />, { api }).dispose;
	for (const name of ["All alerts", "All runs", "All plots"]) {
		const link = page.getByRole("link", { name });
		await expect.element(link).toBeVisible();
		expect(
			link.element().getBoundingClientRect().height,
		).toBeGreaterThanOrEqual(44);
	}
});

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const REPORT = {
	uuid: uuid(61),
	branch: {
		uuid: uuid(1),
		name: "main",
		slug: "main",
		head: {
			uuid: uuid(11),
			version: { number: 7, hash: `9c1f2e4${"0".repeat(33)}` },
		},
	},
	testbed: { uuid: uuid(3), name: "linux", slug: "linux" },
	start_time: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
} as unknown as JsonReport;

const named = (n: number, name: string) => ({
	uuid: uuid(n),
	name,
	slug: name,
});

/** The first page of the report's lines: its groups name every benchmark and the measures each reported. */
const PAGE = {
	groups: [
		{ key: 0, lines: 2, variants: 1, alerts: 0, measures: [0, 1] },
		{ key: 1, lines: 1, variants: 1, alerts: 0, measures: [0] },
		{ key: 2, lines: 1, variants: 1, alerts: 0, measures: [0] },
	],
	benchmarks: [named(5, "blake3"), named(6, "sha256"), named(9, "SHA512")],
	measures: [
		{ ...named(7, "Latency"), units: "nanoseconds (ns)" },
		{ ...named(8, "Throughput"), units: "bytes per second (B/s)" },
	],
	lines: [],
} as unknown as JsonConsoleReport;

const CONSOLE_REPORT = `/v0/console/projects/hashbrown/reports/${uuid(61)}?per_page=1&window=1&points=2`;

/** The start lists with one latest report whose lines answer `lines`, every path asked for kept. */
const withReport = (lines: () => JsonConsoleReport = () => PAGE) => {
	const asked: string[] = [];
	const reading: Api = {
		get: async <T,>(path: string) => {
			asked.push(path);
			const data = path.includes("/reports?")
				? [REPORT]
				: path === CONSOLE_REPORT
					? lines()
					: [];
			return { data: data as T, headers: new Headers() };
		},
		send: api.send,
	};
	return { api: reading, asked };
};

const toggle = () =>
	page
		.getByRole("region", { name: "Latest reports" })
		.getByRole("button", { name: /^main · linux/ });

// Kills a report row that leaves the page, a disclosure that does not say it
// opened or whose search does not take the focus, a search that is not the
// Benchmarks box's (type, arrow keys, Enter), a pick that asks the API again,
// and a pick that does not yield the report's query with its names known.
test("a latest report opens in place to search its benchmarks, and a pick draws one", async () => {
	await page.viewport(1280, 844);
	const { api: reading, asked } = withReport();
	const picked: ExploreQuery[] = [];
	const mounted = mount(
		() => <Start onQuery={(query) => picked.push(query)} />,
		{ api: reading, path: "/hashbrown" },
	);
	dispose = mounted.dispose;
	await expect.element(toggle()).toHaveAttribute("aria-expanded", "false");

	await toggle().click();
	await expect.element(toggle()).toHaveAttribute("aria-expanded", "true");
	expect(mounted.history.get()).toBe("/hashbrown");
	const search = page.getByRole("searchbox", {
		name: "Search benchmarks in report 9c1f2e4",
	});
	await expect.element(search).toHaveFocus();
	const menu = page.getByRole("menu", {
		name: "Benchmarks in report 9c1f2e4",
	});
	await expect.element(menu.getByRole("menuitem")).toHaveLength(3);

	await userEvent.keyboard("sha");
	await expect.element(menu.getByRole("menuitem")).toHaveLength(2);
	await userEvent.keyboard("{ArrowDown}{Enter}");
	expect(picked).toEqual([
		{
			...blankQuery(),
			branches: [{ uuid: uuid(1), head: uuid(11) }],
			testbeds: [{ uuid: uuid(3) }],
			benchmarks: [uuid(6)],
			measures: [uuid(7)],
			report: uuid(61),
		},
	]);
	expect(asked.filter((path) => path.startsWith("/v0/console/"))).toEqual([
		CONSOLE_REPORT,
	]);
	const name = (dimension: Dimension, n: number) =>
		mounted.client.getQueryData(
			nameQuery(reading, "hashbrown", dimension, uuid(n)).queryKey,
		);
	expect([
		name("branches", 1),
		name("testbeds", 3),
		name("benchmarks", 6),
		name("measures", 7),
	]).toEqual([
		{ name: "main" },
		{ name: "linux" },
		{ name: "sha256" },
		{ name: "Latency", units: "nanoseconds (ns)" },
	]);
});

// Kills a report's benchmarks first asked for on the click, which a touch or a
// keyboard waits on, rather than as the pointer or focus arrives.
test("a report's benchmarks are asked for as the pointer or focus arrives, once", async () => {
	for (const arrive of [
		async () => {
			// The pointer rests away from the row, so only the focus can ask.
			await page.getByRole("heading", { name: "Start from" }).hover();
			(toggle().element() as HTMLElement).focus();
		},
		() => toggle().hover(),
	]) {
		const { api: reading, asked } = withReport();
		dispose = mount(() => <Start onQuery={() => {}} />, {
			api: reading,
			path: "/hashbrown",
		}).dispose;
		await expect.element(toggle()).toBeVisible();
		await arrive();
		await expect.poll(() => asked.includes(CONSOLE_REPORT)).toBe(true);
		await toggle().click();
		await expect
			.element(page.getByRole("menuitem", { name: "blake3" }))
			.toBeVisible();
		expect(asked.filter((path) => path === CONSOLE_REPORT)).toHaveLength(1);
		dispose();
		dispose = undefined;
	}
});

/** A report whose job is still running: its lines name no benchmarks until `finish`. */
const pending = () => {
	let finished = false;
	return {
		...withReport(() =>
			finished ? PAGE : { ...PAGE, groups: [], benchmarks: [], measures: [] },
		),
		finish: () => {
			finished = true;
		},
	};
};

const BENCHMARKS = reportBenchmarksQuery(api, "hashbrown", uuid(61)).queryKey;

// Kills an empty answer kept for good, and an expand that does not ask for it
// again: a runner job's report has no benchmarks until the job finishes.
test("a report that answered no benchmarks is asked again on the next expand", async () => {
	await page.viewport(1280, 844);
	const { api: reading, finish } = pending();
	const mounted = mount(() => <Start onQuery={() => {}} />, {
		api: reading,
		path: "/hashbrown",
	});
	dispose = mounted.dispose;
	await toggle().click();
	await expect.poll(() => mounted.client.getQueryData(BENCHMARKS)).toEqual([]);
	await userEvent.keyboard("{Escape}");
	// The focus coming back asks too, so the job finishes once that answer is in.
	await expect.element(toggle()).toHaveFocus();
	await expect.poll(() => mounted.client.isFetching()).toBe(0);
	finish();
	await toggle().click();
	await expect.element(page.getByRole("menuitem")).toHaveLength(3);
});

// Kills a row whose early ask keeps an empty answer, so the expand after the
// job finishes waits on a request the pointer or focus could have made.
test("a report that answered no benchmarks is asked again as the pointer or focus arrives again", async () => {
	const heading = () => page.getByRole("heading", { name: "Start from" });
	for (const [arrive, leave] of [
		[
			async () => {
				await heading().hover();
				(toggle().element() as HTMLElement).focus();
			},
			async () => (toggle().element() as HTMLElement).blur(),
		],
		[() => toggle().hover(), () => heading().hover()],
	] as const) {
		const { api: reading, finish } = pending();
		const mounted = mount(() => <Start onQuery={() => {}} />, {
			api: reading,
			path: "/hashbrown",
		});
		dispose = mounted.dispose;
		await expect.element(toggle()).toBeVisible();
		await arrive();
		await expect
			.poll(() => mounted.client.getQueryData(BENCHMARKS))
			.toEqual([]);
		await leave();
		finish();
		await arrive();
		await expect
			.poll(() => mounted.client.getQueryData(BENCHMARKS))
			.toHaveLength(3);
		dispose();
		dispose = undefined;
	}
});

// Kills a report's benchmarks asked for again once it has some: what a report
// ran never changes once its results land.
test("a report that answered benchmarks is not asked again on the next expand", async () => {
	await page.viewport(1280, 844);
	const { api: reading, asked } = withReport();
	dispose = mount(() => <Start onQuery={() => {}} />, {
		api: reading,
		path: "/hashbrown",
	}).dispose;
	const blake3 = page.getByRole("menuitem", { name: "blake3" });
	await toggle().click();
	await expect.element(blake3).toBeVisible();
	await userEvent.keyboard("{Escape}");
	await expect.element(blake3).not.toBeInTheDocument();
	await toggle().click();
	await expect.element(blake3).toBeVisible();
	expect(asked.filter((path) => path === CONSOLE_REPORT)).toHaveLength(1);
});

// Kills a row only its words open, a hash under the row's click, Escape that
// leaves the row open or drops the focus, and a hash or search too small to tap.
for (const [width, height] of [
	[1280, 844],
	[390, 844],
] as const) {
	test(`a report row opens from anywhere on it, its hash opens the report, at ${width} px`, async () => {
		await page.viewport(width, height);
		dispose = mount(() => <Start onQuery={() => {}} />, {
			api: withReport().api,
			path: "/hashbrown",
		}).dispose;
		await expect.element(toggle()).toBeVisible();
		const button = toggle().element();
		const row = button.parentElement as HTMLElement;
		const hash = page.getByRole("link", { name: "9c1f2e4" });
		await expect
			.element(hash)
			.toHaveAttribute(
				"href",
				`/next/console/projects/hashbrown/reports/${uuid(61)}`,
			);
		const box = row.getBoundingClientRect();
		for (const [x, y] of [
			[box.left + 3, box.top + 3],
			[box.left + box.width / 2, box.bottom - 3],
			[box.right - 3, box.top + box.height / 2],
		] as const) {
			expect(document.elementFromPoint(x, y)?.closest("button")).toBe(button);
		}
		const link = hash.element().getBoundingClientRect();
		expect(
			document
				.elementFromPoint(
					link.left + link.width / 2,
					link.top + link.height / 2,
				)
				?.closest("a"),
		).toBe(hash.element());
		if (width < 768) {
			expect(link.height).toBeGreaterThanOrEqual(44);
		}

		await userEvent.click(row, { position: { x: 3, y: 3 } });
		await expect.element(toggle()).toHaveAttribute("aria-expanded", "true");
		if (width < 768) {
			expect(
				page.getByRole("searchbox").element().getBoundingClientRect().height,
			).toBeGreaterThanOrEqual(44);
		}
		await userEvent.keyboard("{Escape}");
		await expect.element(toggle()).toHaveAttribute("aria-expanded", "false");
		await expect.element(toggle()).toHaveFocus();
	});
}
