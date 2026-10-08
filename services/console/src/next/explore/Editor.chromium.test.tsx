import "./explore.css";
import { createSignal } from "solid-js";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonConsolePerf } from "../../types/bencher";
import type { Api } from "../api";
import { type ExploreQuery, blankQuery } from "../query/query";
import { mount } from "../settings/testing";
import Editor from "./Editor";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const PERF: JsonConsolePerf = {
	window: { start_time: 0, end_time: 1, clamped: false },
	total: 2,
	points: { x: [1], report: [0] },
	reports: [{ uuid: uuid(20), version: 1 }],
	branches: [{ uuid: uuid(1), name: "main", slug: "main", head: uuid(11) }],
	testbeds: [{ uuid: uuid(3), name: "linux", slug: "linux" }],
	benchmarks: [{ uuid: uuid(5), name: "blake3", slug: "blake3" }],
	variants: [
		{ uuid: uuid(30), benchmark: 0, parameters: { simd: "avx2", threads: 1 } },
		{ uuid: uuid(31), benchmark: 0, parameters: { simd: "avx2", threads: 4 } },
		{ uuid: uuid(32), benchmark: 0, parameters: { simd: "sse", threads: 1 } },
	],
	measures: [{ uuid: uuid(7), name: "Latency", slug: "latency", units: "ns" }],
	models: [],
	lines: [0, 1].map((variant) => ({
		branch: 0,
		testbed: 0,
		benchmark: 0,
		variant,
		measure: 0,
		metric: "value",
		series: { y: [1], alerts: [] },
	})),
};

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1) }],
	testbeds: [{ uuid: uuid(3) }],
	benchmarks: [uuid(5)],
	sets: [{ simd: "avx2" }, { simd: "avx2", threads: 1 }],
	measures: [uuid(7)],
};

let cleanup: (() => void) | undefined;
afterEach(() => {
	cleanup?.();
	cleanup = undefined;
});

/** Every path the editor asked the API for, answered by `answer`. */
const reading = (answer: (path: string) => unknown) => {
	const asked: string[] = [];
	const api: Api = {
		get: async <T,>(path: string) => {
			asked.push(path);
			return { data: answer(path) as T, headers: new Headers() };
		},
		send: async () => {
			throw new Error("The editor sends no changes");
		},
	};
	return { api, asked };
};

const render = (
	api: Api,
	initial: ExploreQuery,
	options: {
		perf?: JsonConsolePerf;
		settled?: boolean;
		readOnly?: boolean;
	} = {},
) => {
	const edits: ExploreQuery[] = [];
	const mounted = mount(
		() => {
			const [query, setQuery] = createSignal(initial);
			return (
				<Editor
					query={query()}
					perf={options.perf}
					settled={options.settled ?? true}
					blank={false}
					readOnly={options.readOnly}
					onQuery={(next) => {
						edits.push(next);
						setQuery(next);
					}}
					onFirstBenchmark={() => {}}
				/>
			);
		},
		{ api },
	);
	cleanup = mounted.dispose;
	return edits;
};

// Kills a set's count taken over every variant rather than the ones it
// matches, and a covered set counted as adding the variants its wider set has.
test("each set counts its variants, and one a wider set covers adds none", async () => {
	render(reading(() => []).api, QUERY, { perf: PERF });
	const parameters = page.getByRole("group", { name: "Parameters" });
	await expect
		.element(parameters.getByText("2 variants").first())
		.toBeVisible();
	await expect.element(parameters.getByText("adds 0 variants")).toBeVisible();
	await expect
		.element(parameters.getByText("3 variants"))
		.not.toBeInTheDocument();
});

// Kills names asked for in the first round, beside the plot query, and values
// left as UUIDs once the plot names them.
test("values take their names from the plot, and ask only for those it drew no line of", async () => {
	const { api, asked } = reading((path) =>
		path.includes(uuid(8)) ? { name: "Throughput", units: "B/s" } : [],
	);
	const query = { ...QUERY, measures: [uuid(7), uuid(8)] };
	render(api, query, { perf: PERF, settled: false });
	await expect
		.element(page.getByRole("button", { name: "Remove branch main" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "Remove measure Latency" }))
		.toBeVisible();
	expect(asked).toEqual([]);

	cleanup?.();
	render(api, query, { perf: PERF, settled: true });
	await expect
		.element(page.getByRole("button", { name: "Remove measure Throughput" }))
		.toBeVisible();
	expect(asked).toEqual([`/v0/projects/hashbrown/measures/${uuid(8)}`]);
});

// Kills a search that offers what the box already holds, a typed search that
// never reaches the API, and a pick that does not reach the query.
test("the add control searches, leaves out what the box holds, and adds the pick", async () => {
	const { api, asked } = reading((path) =>
		path.includes("search=feat")
			? [{ uuid: uuid(2), name: "feature" }]
			: [
					{ uuid: uuid(1), name: "main" },
					{ uuid: uuid(2), name: "feature" },
				],
	);
	const edits = render(api, QUERY, { perf: PERF });
	await page.getByRole("button", { name: "Add branch" }).click();
	const menu = page.getByRole("menu", { name: "Add branch" });
	await expect.element(menu.getByRole("menuitem")).toHaveTextContent("feature");
	await userEvent.type(
		page.getByRole("searchbox", { name: "Search branches" }),
		"feat",
	);
	await expect
		.poll(() => asked.some((path) => path.includes("search=feat")))
		.toBe(true);
	await menu.getByRole("menuitem", { name: "feature" }).click();
	expect(edits.at(-1)?.branches).toEqual([
		{ uuid: uuid(1) },
		{ uuid: uuid(2) },
	]);
	await expect
		.element(page.getByRole("button", { name: "Remove branch feature" }))
		.toBeVisible();
});

// Kills suggestions read from the drawn lines alone, which miss the
// benchmark's other variants, a tag counted once however many variants carry
// it, and a tag offered again when a set already holds it alone.
test("the parameters box suggests every variant's tags, counted, but not one already chosen", async () => {
	const { api, asked } = reading((path) =>
		path.includes("/variants")
			? [
					{ simd: "avx2", threads: 1 },
					{ simd: "avx2", threads: 4 },
					{ simd: "sse", threads: 1 },
					{ simd: "neon", threads: 8 },
				].map((parameters, n) => ({ uuid: uuid(40 + n), parameters }))
			: [],
	);
	render(api, { ...QUERY, sets: [{ simd: "avx2" }] }, { perf: PERF });
	await page.getByRole("button", { name: "Add set" }).click();
	const menu = page.getByRole("menu", { name: "Add set" });
	await expect
		.element(menu.getByRole("menuitem", { name: /^simd=neon/ }))
		.toHaveTextContent("1 variant");
	expect(asked).toEqual([
		`/v0/projects/hashbrown/benchmarks/${uuid(5)}/variants?per_page=255`,
	]);
	await expect
		.element(menu.getByRole("menuitem", { name: /^threads=1\b/ }))
		.toHaveTextContent("2 variants");
	expect(
		menu.getByRole("menuitem", { name: /^simd=avx2/ }).elements(),
	).toHaveLength(0);
});

// Kills a set removed by keyboard taking focus with it to the page.
test("removing a set by keyboard moves focus to the next set, then to Add", async () => {
	render(reading(() => []).api, QUERY, { perf: PERF });
	(
		page
			.getByRole("button", { name: "Remove set simd=avx2", exact: true })
			.element() as HTMLElement
	).focus();
	await userEvent.keyboard("{Enter}");
	await expect
		.element(
			page.getByRole("button", { name: "Remove set simd=avx2 threads=1" }),
		)
		.toHaveFocus();
	await userEvent.keyboard("{Enter}");
	await expect
		.element(page.getByRole("button", { name: "Add set" }))
		.toHaveFocus();
});

// Kills a read-only editor that still offers a change.
test("a read-only editor shows the query and nothing that changes it", async () => {
	render(reading(() => []).api, QUERY, { perf: PERF, readOnly: true });
	await expect
		.element(page.getByRole("group", { name: "Branches" }))
		.toHaveTextContent("main");
	expect(page.getByRole("button").elements()).toHaveLength(0);
});
