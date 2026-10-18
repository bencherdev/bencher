import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import "./explore.css";
import { afterEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonConsoleReport,
	JsonConsoleRun,
	JsonConsoleRunSummary,
} from "../../types/bencher";
import type { Api } from "../api";
import { formatWhen } from "../plot/format";
import { type ExploreQuery, blankQuery } from "../query/query";
import { mount } from "../settings/testing";
import { type Dimension, nameQuery } from "./queries";
import Start from "./Start";
import { reportBenchmarksQuery } from "./start";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	vi.useRealTimers();
});

const api: Api = {
	get: async <T,>(path: string) => ({
		data: (path.includes("/runs?") ? { total: 0, runs: [] } : []) as T,
		headers: new Headers(),
	}),
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
	test(`the starting points run Pinned plots, Latest runs, Alerting now at ${width} px`, async () => {
		await page.viewport(width, 844);
		dispose = mount(() => <Start onQuery={() => {}} />, { api }).dispose;
		const lists = ["Pinned plots", "Latest runs", "Alerting now"].map((name) =>
			page.getByRole("region", { name }).element(),
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

const named = (n: number, name: string) => ({
	uuid: uuid(n),
	name,
	slug: name,
});

const STARTED = Date.now() - 24 * 60 * 60 * 1000;
const MINUTE = 60_000;

/** A run of three reports over two testbeds, ten minutes from its first start to its last end. */
const RUN = {
	uuid: uuid(70),
	number: 7,
	hash: `9c1f2e4${"0".repeat(33)}`,
	branch: { ...named(1, "main"), head: uuid(11) },
	start_time: STARTED,
	end_time: STARTED + 10 * MINUTE,
	reports: 3,
	testbeds: ["linux", "macos"],
	testbed_count: 2,
} as unknown as JsonConsoleRunSummary;

/** A run of one report, on one testbed, with no hash. */
const SOLO = {
	...RUN,
	uuid: uuid(71),
	number: 6,
	hash: undefined,
	branch: { ...named(2, "feature"), head: uuid(12) },
	start_time: STARTED - 60 * MINUTE,
	end_time: STARTED - 58 * MINUTE,
	reports: 1,
	testbeds: ["linux"],
	testbed_count: 1,
} as unknown as JsonConsoleRunSummary;

const report = (order: number, n: number, testbed: number, name: string) => ({
	order,
	uuid: uuid(n),
	testbed: named(testbed, name),
	start_time: STARTED + order * 1000,
	end_time: STARTED + order * 1000 + MINUTE,
});

/** RUN's reports in start order: two shards on linux and one on macos. */
const RUN_REPORTS = {
	...RUN,
	total: 3,
	reports: [
		report(1, 61, 3, "linux"),
		report(2, 62, 4, "macos"),
		report(3, 63, 3, "linux"),
	],
} as unknown as JsonConsoleRun;

const SOLO_REPORTS = {
	...SOLO,
	total: 1,
	reports: [report(1, 64, 3, "linux")],
} as unknown as JsonConsoleRun;

const LATENCY = { ...named(7, "Latency"), units: "nanoseconds (ns)" };

/** The first page of the macos report's lines: its groups name every benchmark and the measures each reported. */
const PAGE = {
	groups: [
		{ key: 0, lines: 2, variants: 1, alerts: 0, measures: [0, 1] },
		{ key: 1, lines: 1, variants: 1, alerts: 0, measures: [0] },
		{ key: 2, lines: 1, variants: 1, alerts: 0, measures: [0] },
	],
	benchmarks: [named(5, "blake3"), named(6, "sha256"), named(9, "SHA512")],
	measures: [
		LATENCY,
		{ ...named(8, "Throughput"), units: "bytes per second (B/s)" },
	],
	lines: [],
} as unknown as JsonConsoleReport;

/** The first linux report ran one benchmark of its own. */
const LINUX_PAGE = {
	groups: [{ key: 0, lines: 1, variants: 1, alerts: 0, measures: [0] }],
	benchmarks: [named(10, "crc32c")],
	measures: [LATENCY],
	lines: [],
} as unknown as JsonConsoleReport;

const RUNS = "/v0/console/projects/hashbrown/runs?per_page=4";
const runReports = (n: number) =>
	`/v0/console/projects/hashbrown/runs/${uuid(n)}?per_page=8`;
const reportLines = (n: number) =>
	`/v0/console/projects/hashbrown/reports/${uuid(n)}?per_page=1&window=1&points=2`;
const MACOS = reportLines(62);

/** The start lists with RUN and SOLO, the macos report's lines answering `lines`, every path asked for kept. */
const withRuns = (
	lines: () => JsonConsoleReport = () => PAGE,
	runReportsOf: () => JsonConsoleRun = () => RUN_REPORTS,
	runs: JsonConsoleRunSummary[] = [RUN, SOLO],
) => {
	const asked: string[] = [];
	const reading: Api = {
		get: async <T,>(path: string) => {
			asked.push(path);
			const answers: Record<string, unknown> = {
				[runReports(70)]: runReportsOf(),
				[runReports(71)]: SOLO_REPORTS,
				[MACOS]: lines(),
				[reportLines(61)]: LINUX_PAGE,
			};
			const data = path.startsWith(RUNS)
				? { total: runs.length, runs }
				: (answers[path] ?? []);
			return { data: data as T, headers: new Headers() };
		},
		send: api.send,
	};
	return { api: reading, asked };
};

const latest = () => page.getByRole("region", { name: "Latest runs" });
const runToggle = () =>
	latest().getByRole("button", { name: /^main, run 9c1f2e4/ });
const reports = () =>
	page.getByRole("region", { name: "Reports in run 9c1f2e4", exact: true });
const macos = () => reports().getByRole("button", { name: /^2 main · macos/ });

const opened = async (reading: Api) => {
	const mounted = mount(() => <Start onQuery={() => {}} />, {
		api: reading,
		path: "/hashbrown",
	});
	dispose = mounted.dispose;
	await runToggle().click();
	await expect.element(macos()).toBeVisible();
	return mounted;
};

// Kills a run row that leaves out its run, when it started, its counts, or
// its time, shows a count of one, names a testbed, or links its hash elsewhere
// than the run's page.
test("a run row names its branch, its run, when it started, and its reports and testbeds when there are several", async () => {
	await page.viewport(1280, 844);
	dispose = mount(() => <Start onQuery={() => {}} />, {
		api: withRuns().api,
		path: "/hashbrown",
	}).dispose;
	await expect
		.element(runToggle())
		.toHaveAccessibleName(
			`main, run 9c1f2e4, ${formatWhen(STARTED)}, 3 reports, 2 testbeds, 10m 00s`,
		);
	await expect
		.element(latest().getByRole("button", { name: /^feature/ }))
		.toHaveAccessibleName(
			`feature, run v6, ${formatWhen(STARTED - 60 * MINUTE)}, 2m 00s`,
		);
	expect(latest().element().textContent).not.toContain("linux");
	for (const [name, n] of [
		["9c1f2e4", 70],
		["v6", 71],
	] as const) {
		await expect
			.element(latest().getByRole("link", { name, exact: true }))
			.toHaveAttribute(
				"href",
				`/next/console/projects/hashbrown/runs/${uuid(n)}`,
			);
	}
});

// Kills a run's time read from anything but its first start to its last end,
// or drawn anywhere but the row's end beside its caret, a hash away from the
// branch name, and a long branch name that pushes the hash out of the row.
for (const width of [1280, 390] as const) {
	test(`a run's time ends its row beside the caret, and a long branch name leaves its hash in view, at ${width} px`, async () => {
		await page.viewport(width, 844);
		const long = {
			...RUN,
			branch: { ...RUN.branch, name: `feature/${"x".repeat(80)}` },
		} as unknown as JsonConsoleRunSummary;
		for (const run of [RUN, long]) {
			dispose?.();
			dispose = mount(() => <Start onQuery={() => {}} />, {
				api: withRuns(undefined, undefined, [run]).api,
				path: "/hashbrown",
			}).dispose;
			const time = latest().getByText("10m 00s", { exact: true });
			await expect.element(time).toBeVisible();
			const hash = latest()
				.getByRole("link", { name: "9c1f2e4", exact: true })
				.element();
			const row = hash.closest(".ex-rhead") as HTMLElement;
			const caret = row.lastElementChild as Element;
			const at = time.element().getBoundingClientRect();
			const link = hash.getBoundingClientRect();
			expect(caret.getBoundingClientRect().left - at.right).toBeLessThanOrEqual(
				12,
			);
			expect(link.right).toBeLessThanOrEqual(at.left);
			// The hash sits right beside the branch name, and a name too long for
			// its line ends before the hash rather than under it.
			const name = (
				(hash.previousElementSibling as Element).firstElementChild as Element
			).getBoundingClientRect();
			expect(link.left - name.right).toBeLessThanOrEqual(12);
			expect(name.right).toBeLessThanOrEqual(link.left);
			expect(
				document.elementFromPoint(
					link.left + link.width / 2,
					link.top + link.height / 2,
				),
			).toBe(hash);
		}
	});
}

// Kills an opened run that leaves the page, a disclosure that does not say it
// opened, its reports out of start order or without their place in the run,
// their branch, or their testbed, and an open that asks for more than one
// answer.
test("an opened run lists its reports in start order as Branch and Testbed rows", async () => {
	await page.viewport(1280, 844);
	const { api: reading, asked } = withRuns();
	const mounted = await opened(reading);
	await expect.element(runToggle()).toHaveAttribute("aria-expanded", "true");
	expect(mounted.history.get()).toBe("/hashbrown");
	expect(
		reports()
			.getByRole("button")
			.elements()
			.map((each) => each.textContent),
	).toEqual(["1main· linux", "2main· macos", "3main· linux"]);
	expect(asked.filter((path) => path.includes("/runs/"))).toEqual([
		runReports(70),
	]);
});

// Kills an opened run that lists only some of its reports with no way to the
// rest, a way too small to tap on a phone, or one offered when it lists them
// all.
test("an opened run with more reports than it lists links to its page for the rest", async () => {
	await page.viewport(390, 844);
	const many = {
		...RUN_REPORTS,
		total: 10,
		reports: Array.from({ length: 8 }, (_, n) =>
			report(n + 1, 80 + n, 3, "linux"),
		),
	} as unknown as JsonConsoleRun;
	dispose = mount(() => <Start onQuery={() => {}} />, {
		api: withRuns(undefined, () => many).api,
		path: "/hashbrown",
	}).dispose;
	await runToggle().click();
	await expect.element(reports().getByRole("button")).toHaveLength(8);
	const rest = reports().getByRole("link", { name: "All 10 reports" });
	await expect
		.element(rest)
		.toHaveAttribute(
			"href",
			`/next/console/projects/hashbrown/runs/${uuid(70)}`,
		);
	expect(rest.element().getBoundingClientRect().height).toBeGreaterThanOrEqual(
		44,
	);
	await latest()
		.getByRole("button", { name: /^feature/ })
		.click();
	await expect
		.element(
			page
				.getByRole("region", { name: "Reports in run v6", exact: true })
				.getByRole("button"),
		)
		.toHaveLength(1);
	expect(
		latest()
			.getByRole("link", { name: /^All \d+ reports$/ })
			.elements(),
	).toEqual([]);
});

// Kills a caret that does not turn as its run or report opens.
test("a run's and a report's caret turn as they open", async () => {
	await page.viewport(1280, 844);
	dispose = mount(() => <Start onQuery={() => {}} />, {
		api: withRuns().api,
		path: "/hashbrown",
	}).dispose;
	const turned = (toggle: () => ReturnType<typeof runToggle>) =>
		getComputedStyle(
			(toggle().element().closest(".ex-rhead") as Element)
				.lastElementChild as Element,
		).transform !== "none";
	await expect.element(runToggle()).toBeVisible();
	expect(turned(runToggle)).toBe(false);
	await runToggle().click();
	await expect.element(macos()).toBeVisible();
	expect([turned(runToggle), turned(macos)]).toEqual([true, false]);
	await macos().click();
	await expect.element(macos()).toHaveAttribute("aria-expanded", "true");
	expect(turned(macos)).toBe(true);
});

// Kills an opened run that holds a place for more or fewer reports than it
// will list while they load, which moves the rows below once they arrive.
test("an opened run holds a row for each report it will list while they load", async () => {
	await page.viewport(1280, 844);
	const twelve = { ...RUN, reports: 12 } as unknown as JsonConsoleRunSummary;
	for (const [run, held] of [
		[RUN, 3],
		[twelve, 8],
	] as const) {
		const waiting: Api = {
			get: async <T,>(path: string) =>
				path.startsWith(RUNS)
					? { data: { total: 1, runs: [run] } as T, headers: new Headers() }
					: new Promise<never>(() => {}),
			send: api.send,
		};
		dispose?.();
		dispose = mount(() => <Start onQuery={() => {}} />, {
			api: waiting,
			path: "/hashbrown",
		}).dispose;
		await runToggle().click();
		await expect.element(reports()).toBeVisible();
		expect(reports().element().querySelectorAll(".ex-skel")).toHaveLength(held);
	}
});

// Kills a run's reports first asked for on the click, which a touch or a
// keyboard waits on, rather than as the pointer or focus arrives.
test("a run's reports are asked for as the pointer or focus arrives, once", async () => {
	for (const arrive of [
		async () => {
			// The pointer rests away from the row, so only the focus can ask.
			await page.getByRole("heading", { name: "Start from" }).hover();
			(runToggle().element() as HTMLElement).focus();
		},
		() => runToggle().hover(),
	]) {
		const { api: reading, asked } = withRuns();
		dispose = mount(() => <Start onQuery={() => {}} />, {
			api: reading,
			path: "/hashbrown",
		}).dispose;
		await expect.element(runToggle()).toBeVisible();
		await arrive();
		await expect.poll(() => asked.includes(runReports(70))).toBe(true);
		await runToggle().click();
		await expect.element(macos()).toBeVisible();
		expect(asked.filter((path) => path === runReports(70))).toHaveLength(1);
		dispose();
		dispose = undefined;
	}
});

// Kills a report row that does not open in place, a search that does not take
// the focus or is not the Benchmarks box's (type, arrow keys, Enter), a pick
// that asks the API again, and a pick that does not draw the report's head,
// its one testbed, and its measures with their names known.
test("a report opens in place to search its benchmarks, and a pick draws one", async () => {
	await page.viewport(1280, 844);
	const { api: reading, asked } = withRuns();
	const picked: ExploreQuery[] = [];
	const mounted = mount(
		() => <Start onQuery={(query) => picked.push(query)} />,
		{ api: reading, path: "/hashbrown" },
	);
	dispose = mounted.dispose;
	await runToggle().click();
	await expect.element(macos()).toHaveAttribute("aria-expanded", "false");
	await macos().click();
	await expect.element(macos()).toHaveAttribute("aria-expanded", "true");
	const search = page.getByRole("searchbox", {
		name: "Search benchmarks in report 2 on macos",
		exact: true,
	});
	await expect.element(search).toHaveFocus();
	const menu = page.getByRole("menu", {
		name: "Benchmarks in report 2 on macos",
		exact: true,
	});
	await expect.element(menu.getByRole("menuitem")).toHaveLength(3);

	await userEvent.keyboard("sha");
	await expect.element(menu.getByRole("menuitem")).toHaveLength(2);
	await userEvent.keyboard("{ArrowDown}{Enter}");
	expect(picked).toEqual([
		{
			...blankQuery(),
			branches: [{ uuid: uuid(1), head: uuid(11) }],
			testbeds: [{ uuid: uuid(4) }],
			benchmarks: [uuid(6)],
			measures: [uuid(7)],
			report: uuid(62),
		},
	]);
	expect(asked.filter((path) => path.startsWith("/v0/console/"))).toEqual([
		expect.stringMatching(
			/^\/v0\/console\/projects\/hashbrown\/runs\?per_page=4/,
		),
		runReports(70),
		MACOS,
	]);
	const name = (dimension: Dimension, n: number) =>
		mounted.client.getQueryData(
			nameQuery(reading, "hashbrown", dimension, uuid(n)).queryKey,
		);
	expect([
		name("branches", 1),
		name("testbeds", 4),
		name("benchmarks", 6),
		name("measures", 7),
	]).toEqual([
		{ name: "main" },
		{ name: "macos" },
		{ name: "sha256" },
		{ name: "Latency", units: "nanoseconds (ns)" },
	]);
});

// Kills a report's benchmarks cached under a key that leaves out the report,
// so the second report opened shows the first one's.
test("each report opens to its own benchmarks", async () => {
	await page.viewport(1280, 844);
	await opened(withRuns().api);
	await macos().click();
	await expect
		.element(page.getByRole("menuitem", { name: "SHA512" }))
		.toBeVisible();
	await userEvent.keyboard("{Escape}");
	await reports()
		.getByRole("button", { name: /^1 main · linux/ })
		.click();
	const items = page
		.getByRole("menu", { name: "Benchmarks in report 1 on linux", exact: true })
		.getByRole("menuitem");
	await expect.element(items).toHaveLength(1);
	await expect.element(items).toHaveAccessibleName("crc32c");
});

// Kills a report's benchmarks first asked for on the click, which a touch or a
// keyboard waits on, rather than as the pointer or focus arrives.
test("a report's benchmarks are asked for as the pointer or focus arrives, once", async () => {
	for (const arrive of [
		async () => {
			await page.getByRole("heading", { name: "Start from" }).hover();
			(macos().element() as HTMLElement).focus();
		},
		() => macos().hover(),
	]) {
		const { api: reading, asked } = withRuns();
		await opened(reading);
		await arrive();
		await expect.poll(() => asked.includes(MACOS)).toBe(true);
		await macos().click();
		await expect
			.element(page.getByRole("menuitem", { name: "blake3" }))
			.toBeVisible();
		expect(asked.filter((path) => path === MACOS)).toHaveLength(1);
		dispose?.();
		dispose = undefined;
	}
});

/** A report whose job is still running: its lines name no benchmarks until `finish`. */
const pending = () => {
	let finished = false;
	return {
		...withRuns(() =>
			finished ? PAGE : { ...PAGE, groups: [], benchmarks: [], measures: [] },
		),
		finish: () => {
			finished = true;
		},
	};
};

const BENCHMARKS = reportBenchmarksQuery(api, "hashbrown", uuid(62)).queryKey;

// Kills an empty answer kept for good, and an expand that does not ask for it
// again: a runner job's report has no benchmarks until the job finishes.
test("a report that answered no benchmarks is asked again on the next expand", async () => {
	await page.viewport(1280, 844);
	const { api: reading, finish } = pending();
	const mounted = await opened(reading);
	await macos().click();
	await expect.poll(() => mounted.client.getQueryData(BENCHMARKS)).toEqual([]);
	await userEvent.keyboard("{Escape}");
	// The focus coming back asks too, so the job finishes once that answer is in.
	await expect.element(macos()).toHaveFocus();
	await expect.poll(() => mounted.client.isFetching()).toBe(0);
	finish();
	await macos().click();
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
				(macos().element() as HTMLElement).focus();
			},
			async () => (macos().element() as HTMLElement).blur(),
		],
		[() => macos().hover(), () => heading().hover()],
	] as const) {
		const { api: reading, finish } = pending();
		const mounted = await opened(reading);
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
		dispose?.();
		dispose = undefined;
	}
});

// Kills a report's benchmarks asked for again once it has some: what a report
// ran never changes once its results land.
test("a report that answered benchmarks is not asked again on the next expand", async () => {
	await page.viewport(1280, 844);
	const { api: reading, asked } = withRuns();
	await opened(reading);
	const blake3 = page.getByRole("menuitem", { name: "blake3" });
	await macos().click();
	await expect.element(blake3).toBeVisible();
	await userEvent.keyboard("{Escape}");
	await expect.element(blake3).not.toBeInTheDocument();
	await macos().click();
	await expect.element(blake3).toBeVisible();
	expect(asked.filter((path) => path === MACOS)).toHaveLength(1);
});

// Kills a run or report row only its words open, a hash the row's click
// covers or its line clips at its edges, Escape that leaves the report open or
// drops the focus, and a hash, search, or benchmark too small to tap.
for (const [width, height] of [
	[1280, 844],
	[390, 844],
] as const) {
	test(`a run or report row opens from anywhere on it, its hash opens the run, at ${width} px`, async () => {
		await page.viewport(width, height);
		dispose = mount(() => <Start onQuery={() => {}} />, {
			api: withRuns().api,
			path: "/hashbrown",
		}).dispose;
		await expect.element(runToggle()).toBeVisible();
		const hash = page.getByRole("link", { name: "9c1f2e4", exact: true });
		const link = hash.element().getBoundingClientRect();
		expect(
			document
				.elementFromPoint(
					link.left + link.width / 2,
					link.top + link.height / 2,
				)
				?.closest("a"),
		).toBe(hash.element());
		// The hash is reached at its own top and bottom edges, which its line never clips.
		for (const y of [link.top + 2, link.bottom - 2]) {
			expect(
				document.elementFromPoint(link.left + link.width / 2, y)?.closest("a"),
			).toBe(hash.element());
		}
		if (width < 768) {
			expect(link.height).toBeGreaterThanOrEqual(44);
		}
		for (const toggle of [runToggle, macos]) {
			const button = toggle().element();
			const row = button.closest(".ex-rhead") as HTMLElement;
			const box = row.getBoundingClientRect();
			for (const [x, y] of [
				[box.left + 3, box.top + 3],
				[box.left + box.width / 2, box.bottom - 3],
				[box.right - 3, box.top + box.height / 2],
			] as const) {
				expect(document.elementFromPoint(x, y)?.closest("button")).toBe(button);
			}
			await userEvent.click(row, { position: { x: 3, y: 3 } });
			await expect.element(toggle()).toHaveAttribute("aria-expanded", "true");
		}
		if (width < 768) {
			for (const target of [
				page.getByRole("searchbox").element(),
				...page.getByRole("menuitem").elements(),
			]) {
				expect(target.getBoundingClientRect().height).toBeGreaterThanOrEqual(
					44,
				);
			}
		}
		await userEvent.keyboard("{Escape}");
		await expect.element(macos()).toHaveAttribute("aria-expanded", "false");
		await expect.element(macos()).toHaveFocus();
	});
}

/** RUN while its macos shard has run 48 s by `now` and its last linux shard waits for a runner. */
const running = (now: number) => {
	const jobs = {
		queued: 1,
		running: 1,
		failed: 0,
		canceled: 0,
		since: now - 48_000,
	};
	return {
		run: { ...RUN, jobs } as unknown as JsonConsoleRunSummary,
		reports: {
			...RUN_REPORTS,
			jobs,
			reports: [
				report(1, 61, 3, "linux"),
				{
					...report(2, 62, 4, "macos"),
					job: { status: "running", created: STARTED, started: now - 48_000 },
				},
				{
					...report(3, 63, 3, "linux"),
					job: { status: "queued", created: STARTED },
				},
			],
		} as unknown as JsonConsoleRun,
	};
};

// Kills a pending run row that shows its time rather than its status, a run
// button whose name leaves the status out, a report a runner has not finished
// offered as a benchmark search with nothing to search, and a finished report
// of the same run that loses its search.
test("a run with jobs not done shows its status, and its unfinished reports show theirs in place of a search", async () => {
	await page.viewport(1280, 844);
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(STARTED + 60 * MINUTE);
	const shown = running(Date.now());
	const { api: reading } = withRuns(
		() => LINUX_PAGE,
		() => shown.reports,
		[shown.run],
	);
	dispose = mount(() => <Start onQuery={() => {}} />, {
		api: reading,
		path: "/hashbrown",
	}).dispose;
	const toggle = latest().getByRole("button", {
		name: /^main, run 9c1f2e4, .*, Running 48s$/,
	});
	await expect.element(toggle).toBeVisible();
	expect(latest().element().querySelector(".ex-n")).toBeNull();
	await toggle.click();
	await expect
		.element(reports().getByRole("button", { name: /^1 main · linux/ }))
		.toBeVisible();
	expect(
		reports()
			.getByRole("button", { name: /^(2|3) main/ })
			.elements(),
	).toHaveLength(0);
	expect(
		[...reports().element().querySelectorAll(".rn-st")].map(
			(pill) => pill.textContent,
		),
	).toEqual(["Running 48s", "Queued"]);
});

// Kills an opened run's unfinished report row whose order, branch, testbed, and
// status stack rather than share its one line, a status that does not end the
// row as a finished row's caret does, and a row taller than a finished one.
for (const width of [1280, 390] as const) {
	test(`an opened run's unfinished report rows hold one line, as tall as a finished one, at ${width} px`, async () => {
		await page.viewport(width, 844);
		vi.useFakeTimers({ toFake: ["Date"] });
		vi.setSystemTime(STARTED + 60 * MINUTE);
		const shown = running(Date.now());
		const { api: reading } = withRuns(
			() => LINUX_PAGE,
			() => shown.reports,
			[shown.run],
		);
		dispose = mount(() => <Start onQuery={() => {}} />, {
			api: reading,
			path: "/hashbrown",
		}).dispose;
		await latest()
			.getByRole("button", { name: /^main, run 9c1f2e4/ })
			.click();
		await expect.element(reports().getByText("Queued")).toBeVisible();
		const finished = reports()
			.getByRole("button", { name: /^1 main · linux/ })
			.element()
			.closest(".ex-rhead") as HTMLElement;
		const rows = [...reports().element().querySelectorAll(".rn-st")].map(
			(pill) => pill.closest(".ex-rhead") as HTMLElement,
		);
		expect(rows).toHaveLength(2);
		for (const row of rows) {
			const parts = [
				...row.querySelectorAll(".ex-sline > *"),
				row.querySelector(".rn-st"),
			].map((part) => (part as Element).getBoundingClientRect());
			expect(parts).toHaveLength(4);
			const top = Math.max(...parts.map((box) => box.top));
			const bottom = Math.min(...parts.map((box) => box.bottom));
			expect(top).toBeLessThan(bottom);
			const pill = parts[3] as DOMRect;
			expect(
				row.getBoundingClientRect().right - pill.right,
			).toBeLessThanOrEqual(
				finished.getBoundingClientRect().right -
					(finished.lastElementChild as Element).getBoundingClientRect().right +
					1,
			);
			expect(row.getBoundingClientRect().height).toBeCloseTo(
				finished.getBoundingClientRect().height,
				0,
			);
		}
	});
}

/** RUN running in blank Explore, mounted with its client, at the hour after it started. */
const mountRunning = async () => {
	await page.viewport(1280, 844);
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(STARTED + 60 * MINUTE);
	const shown = running(Date.now());
	const { api: reading } = withRuns(
		() => LINUX_PAGE,
		() => shown.reports,
		[shown.run],
	);
	const mounted = mount(() => <Start onQuery={() => {}} />, {
		api: reading,
		path: "/hashbrown",
	});
	dispose = mounted.dispose;
	return mounted.client;
};

const runningToggle = () =>
	latest().getByRole("button", { name: /^main, run 9c1f2e4/ });

// Kills blank Explore's latest runs whose running time stands still as their
// answers come in.
test("a latest run's running time moves on with each answer", async () => {
	const client = await mountRunning();
	await expect.element(runningToggle()).toHaveAccessibleName(/, Running 48s$/);
	vi.setSystemTime(STARTED + 63 * MINUTE);
	await client.refetchQueries();
	await expect.element(runningToggle()).toHaveAccessibleName(/, Running 3m$/);
});

// Kills an opened run's report rows whose running time stands still as their
// answers come in.
test("an opened run's running report moves its time on with each answer", async () => {
	const client = await mountRunning();
	await runningToggle().click();
	await expect.element(reports().getByText("Running 48s")).toBeVisible();
	vi.setSystemTime(STARTED + 63 * MINUTE);
	await client.refetchQueries();
	await expect.element(reports().getByText("Running 3m")).toBeVisible();
});
