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
		perf?: JsonConsolePerf | undefined;
		settled?: boolean;
		readOnly?: boolean;
		versionZero?: boolean;
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
					readOnly={options.readOnly}
					versionZero={options.versionZero}
					onQuery={(next) => {
						edits.push(next);
						setQuery(next);
					}}
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

// Kills a search that offers what the box already holds, a search that asks
// anything but the console's most recently used, a typed search that never
// reaches the API, and a pick that does not reach the query.
test("the add control searches, leaves out what the box holds, and adds the pick", async () => {
	const { api, asked } = reading((path) =>
		path.includes("search=feat")
			? { branches: [{ uuid: uuid(2), name: "feature" }] }
			: {
					branches: [
						{ uuid: uuid(1), name: "main" },
						{ uuid: uuid(2), name: "feature" },
					],
				},
	);
	const edits = render(api, QUERY, { perf: PERF });
	await page.getByRole("button", { name: "Add branch" }).click();
	const menu = page.getByRole("menu", { name: "Add branch" });
	await expect.element(menu.getByRole("menuitem")).toHaveTextContent("feature");
	await userEvent.type(
		page.getByRole("searchbox", { name: "Search branches" }),
		"feat",
	);
	const searches =
		"/v0/console/projects/hashbrown/branches?per_page=32&sort=last_used";
	await expect.poll(() => asked).toContain(`${searches}&search=feat`);
	expect(asked[0]).toBe(searches);
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
// it, a tag offered again when a set already holds it alone, and tags not
// ranked by their most recent variant.
test("the parameters box suggests every variant's tags, counted and most recent first, but not one already chosen", async () => {
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
		`/v0/projects/hashbrown/benchmarks/${uuid(5)}/variants?per_page=255&sort=last_used`,
	]);
	await expect
		.element(menu.getByRole("menuitem", { name: /^threads=1\b/ }))
		.toHaveTextContent("2 variants");
	expect(
		menu.getByRole("menuitem", { name: /^simd=avx2/ }).elements(),
	).toHaveLength(0);
	expect(
		menu
			.getByRole("menuitem")
			.elements()
			.map(({ firstChild }) => firstChild?.textContent),
	).toEqual(["threads=1", "threads=4", "simd=sse", "simd=neon", "threads=8"]);
});

// Kills an empty value shown bare in a set and in a suggestion, which reads as no value.
test("the parameters box shows an empty value as quotes", async () => {
	const { api } = reading((path) =>
		path.includes("/variants")
			? [{ env: "" }, { env: "a" }].map((parameters, n) => ({
					uuid: uuid(40 + n),
					parameters,
				}))
			: [],
	);
	render(api, { ...QUERY, sets: [{ env: "a" }] }, { perf: PERF });
	await expect
		.element(page.getByRole("button", { name: "Remove set env=a" }))
		.toBeVisible();
	await page.getByRole("button", { name: "Add set" }).click();
	await expect
		.element(
			page
				.getByRole("menu", { name: "Add set" })
				.getByRole("menuitem", { name: /^env=""/ }),
		)
		.toHaveTextContent("1 variant");
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

// Kills a Parameters hint sized to its text, whose box moves when a single
// variant project's "0 variants" becomes "1 variant" as the plot arrives.
test("the parameters hint holds its place when its count arrives at 900 px", async () => {
	await page.viewport(900, 900);
	const [perf, setPerf] = createSignal<JsonConsolePerf>();
	render(
		reading(() => []).api,
		{ ...QUERY, sets: [] },
		{
			get perf() {
				return perf();
			},
		},
	);
	const parameters = page.getByRole("group", { name: "Parameters" });
	const hint = parameters.getByText(/^\d+ variants?$/);
	await expect.element(hint).toHaveTextContent("0 variants");
	const place = () => hint.element().getBoundingClientRect().toJSON();
	const before = place();
	setPerf({
		...PERF,
		variants: PERF.variants.slice(0, 1),
		lines: PERF.lines.slice(0, 1),
	});
	await expect.element(hint).toHaveTextContent("1 variant");
	expect(place()).toEqual(before);
});

const LOCKED = "Filtering by parameters needs a version 1 project.";

// Kills a parameters box a version 0 project can add a set to, which its
// variants have no parameters to match, one disabled without a mark or the
// tooltip that says why, and one locked on a version 1 project.
test("on a version 0 project the parameters box is locked, and says why", async () => {
	await page.viewport(1280, 800);
	const { api, asked } = reading(() => []);
	const edits = render(
		api,
		{ ...QUERY, sets: [] },
		{
			perf: PERF,
			versionZero: true,
		},
	);
	const parameters = page.getByRole("group", { name: "Parameters" });
	const add = parameters.getByRole("button", { name: "Add set" });
	await expect.element(add).toHaveAttribute("aria-disabled", "true");
	await expect.element(add).toHaveAccessibleDescription(LOCKED);
	expect(add.element().querySelector('[data-icon="read-only"]')).not.toBeNull();
	await add.hover();
	await expect.element(page.getByRole("tooltip")).toHaveTextContent(LOCKED);
	(add.element() as HTMLButtonElement).click();
	await expect.element(page.getByRole("menu")).not.toBeInTheDocument();
	expect(edits).toEqual([]);
	expect(asked.filter((path) => path.includes("/variants"))).toEqual([]);
	await expect.element(parameters.getByText("every variant")).toBeVisible();
	cleanup?.();

	render(api, QUERY, { perf: PERF });
	const open = page
		.getByRole("group", { name: "Parameters" })
		.getByRole("button", { name: "Add set" });
	await expect.element(open).not.toHaveAttribute("aria-disabled");
	await open.click();
	await expect.element(page.getByRole("menu")).toBeVisible();
});
