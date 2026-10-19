import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../explore/explore.css";
import "../plot/plot.css";
import "./public.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonConsolePerf } from "../../types/bencher";
import Plot from "../plot/Plot";
import { type ExploreQuery, blankQuery, decodeQuery } from "../query/query";
import Public, { type PublicProps } from "./Public";

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
	lines: [0, 1, 2].map((variant) => ({
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
	sets: [{ simd: "avx2" }],
	measures: [uuid(7)],
	xAxis: "version",
};

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
	document.documentElement.removeAttribute("data-reader");
	document.documentElement.removeAttribute("data-theme");
});

const show = (props: Partial<PublicProps> = {}) => {
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<Public
				slug="hashbrown"
				name="Hashbrown"
				query={QUERY}
				signIn="/auth/login?back=x"
				view="plot"
				answer={PERF}
				{...props}
			/>
		),
		root,
	);
};

// Kills boxes that offer an edit, values left unnamed though the plot named
// them, and a set counted over every variant rather than those it matches.
test("the query shows read only, named from the plot", async () => {
	await page.viewport(1280, 900);
	show();
	await userEvent.click(page.getByRole("button", { name: "Show query" }));
	const box = (name: string) => page.getByRole("group", { name });
	await expect.element(box("Branches").getByText("main")).toBeVisible();
	await expect.element(box("Testbeds").getByText("linux")).toBeVisible();
	await expect.element(box("Benchmarks").getByText("blake3")).toBeVisible();
	await expect
		.element(box("Parameters").getByText("2 variants"))
		.toBeVisible();
	await expect.element(box("Measures").getByText("Latency")).toBeVisible();
	await expect.element(box("Metrics").getByText("every metric")).toBeVisible();
	expect(
		page.getByRole("button", { name: /^(Add|Remove)/ }).elements(),
	).toHaveLength(0);
	await expect
		.element(page.getByRole("button", { name: "Hide query" }))
		.toHaveAttribute("aria-expanded", "true");
});

// Kills a custom window the API would cut short, and a view change lost
// rather than handed back as the query.
test("the window offers three presets, and a pick changes the query", async () => {
	await page.viewport(1280, 900);
	const changes: ExploreQuery[] = [];
	show({ onQuery: (next) => changes.push(next) });
	const windows = page.getByRole("radiogroup", { name: "Window" });
	expect(
		windows
			.getByRole("radio")
			.elements()
			.map((radio) => radio.parentElement?.textContent),
	).toEqual(["1w", "4w", "3m"]);
	await userEvent.click(windows.getByText("3m"));
	expect(changes.map(({ window }) => window)).toEqual([
		{ seconds: 92 * 24 * 60 * 60 },
	]);
});

// Kills a member's action shown to a reader who is not signed in, Log in
// shown to one who is, and Open in console on another query than the one drawn.
test("only a stored reader sees Open in console, on this query", async () => {
	await page.viewport(1280, 900);
	show();
	await expect
		.element(page.getByRole("link", { name: "Log in" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("link", { name: "Open in console" }))
		.not.toBeInTheDocument();

	document.documentElement.setAttribute("data-reader", "");
	const open = page.getByRole("link", { name: "Open in console" });
	await expect.element(open).toBeVisible();
	await expect
		.element(page.getByRole("link", { name: "Log in" }))
		.not.toBeInTheDocument();
	const href = new URL(
		open.element().getAttribute("href") ?? "",
		"https://bencher.dev",
	);
	expect(href.pathname).toBe("/console/projects/hashbrown/explore");
	expect(decodeQuery(href.search)).toEqual(QUERY);
});

// Kills a refused query that drops the lines drawn or offers no way to ask again.
test("a refused query keeps what was drawn and Retry asks again", async () => {
	await page.viewport(1280, 900);
	let retried = 0;
	show({
		refused: "The plot did not load",
		onRetry: () => {
			retried++;
		},
		plot: (await import("../plot/Plot")).default,
	});
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("The plot did not load");
	await expect
		.element(page.getByRole("figure"))
		.toHaveAccessibleName("3 lines");
	await userEvent.click(page.getByRole("button", { name: "Retry" }));
	expect(retried).toBe(1);
});

const DAY = 24 * 60 * 60;
const region = () => page.getByRole("region", { name: "Query, read only" });

// Kills a window the API cut drawn without saying so, a cut said of a window
// drawn whole, and a cut whose arrival moves the page.
test("a window the API cut says so in the plot's axes row, and its arrival moves nothing", async () => {
	for (const width of [1280, 390]) {
		await page.viewport(width, 900);
		const [answer, setAnswer] = createSignal<JsonConsolePerf>(PERF);
		const root = document.createElement("div");
		root.className = "console";
		document.body.append(root);
		dispose = render(
			() => (
				<Public
					slug="hashbrown"
					name="Hashbrown"
					query={QUERY}
					signIn="/auth/login?back=x"
					view="plot"
					answer={answer()}
					plot={Plot}
				/>
			),
			root,
		);
		const figure = page.getByRole("figure");
		const cut = page.getByText(/3 months/);
		await expect.element(figure).toHaveAccessibleName("3 lines");
		expect(cut.query()).toBeNull();
		const place = () => {
			const { x, y, height } = figure.element().getBoundingClientRect();
			return { x, y, height };
		};
		const before = place();
		setAnswer({ ...PERF, window: { ...PERF.window, clamped: true } });
		await expect.element(cut).toBeVisible();
		expect(place()).toEqual(before);
		setAnswer(PERF);
		await expect.poll(() => cut.query()).toBeNull();
		dispose();
		dispose = undefined;
		document.body.replaceChildren();
	}
});

// Kills a preset shown as the window of a link that ends earlier, and a pick
// there that keeps the link's end, which never reaches the present.
test("a window ending earlier checks no preset, and a pick ends now", async () => {
	await page.viewport(1280, 900);
	const changes: ExploreQuery[] = [];
	show({
		query: { ...QUERY, window: { seconds: 92 * DAY, end: Date.UTC(2026, 6, 1) } },
		onQuery: (next) => changes.push(next),
	});
	const windows = page.getByRole("radiogroup", { name: "Window" });
	expect(
		windows
			.getByRole("radio")
			.elements()
			.filter((radio) => (radio as HTMLInputElement).checked),
	).toHaveLength(0);
	await userEvent.click(windows.getByText("3m"));
	await userEvent.click(windows.getByText("1w"));
	expect(changes.map(({ window }) => window)).toEqual([
		{ seconds: 92 * DAY },
		{ seconds: 7 * DAY },
	]);
});

// Kills a Metrics box that says every metric when the query names one.
test("the Metrics box names the query's metrics", async () => {
	await page.viewport(1280, 900);
	show({ query: { ...QUERY, metrics: ["value"] } });
	await userEvent.click(page.getByRole("button", { name: "Show query" }));
	const metrics = page.getByRole("group", { name: "Metrics" });
	await expect.element(metrics.getByText("value", { exact: true })).toBeVisible();
	expect(metrics.getByText("every metric").elements()).toHaveLength(0);
});

// Kills a first read the API refused that leaves the summary waiting on names
// that never come.
test("a refused first read shows the summary", async () => {
	await page.viewport(1280, 900);
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	// No answer at all: a spread `answer: undefined` would leave the default one.
	dispose = render(
		() => (
			<Public
				slug="hashbrown"
				name="Hashbrown"
				query={QUERY}
				signIn="/auth/login?back=x"
				view="plot"
				refused="The API refused this query."
			/>
		),
		root,
	);
	for (const key of [
		"branches",
		"testbeds",
		"benchmarks",
		"parameters",
		"measures",
		"metrics",
	]) {
		await expect
			.element(region().getByText(key, { exact: true }))
			.toBeVisible();
	}
});

// Kills an open query that keeps its summary, or whose chevron never turns.
test("Show query takes the summary away for the boxes and turns its chevron", async () => {
	await page.viewport(1280, 900);
	show();
	const chevron = () =>
		getComputedStyle(
			region().getByRole("button", { name: /query$/ }).element().querySelector(
				".ui-icon",
			) as Element,
		).transform;
	const closed = chevron();
	await userEvent.click(page.getByRole("button", { name: "Show query" }));
	await expect
		.element(page.getByRole("group", { name: "Branches" }))
		.toBeVisible();
	expect(region().getByText("branches", { exact: true }).elements()).toHaveLength(
		0,
	);
	expect(chevron()).not.toBe(closed);
	await userEvent.click(page.getByRole("button", { name: "Hide query" }));
	await expect
		.element(region().getByText("branches", { exact: true }))
		.toBeVisible();
	expect(chevron()).toBe(closed);
});

// Kills a signed in reader the project refuses who is told to log in or sign
// up, and a reader who is not signed in who is not.
test("a locked page asks only a reader who is not signed in to log in", async () => {
	await page.viewport(1280, 900);
	show({ view: "locked" });
	const locked = page.getByRole("heading", {
		level: 1,
		name: "Private or not found",
	});
	await expect.element(locked).toBeVisible();
	await expect
		.element(page.getByRole("main").getByRole("link", { name: "Log in" }))
		.toHaveAttribute("href", "/auth/login?back=x");

	document.documentElement.setAttribute("data-reader", "");
	await expect.element(locked).toBeVisible();
	for (const name of ["Log in", "Sign up"]) {
		expect(page.getByRole("link", { name }).elements()).toHaveLength(0);
	}
});

/** What the bar shows a reader, each control by its name, in order. */
const held = () =>
	[...document.querySelectorAll<HTMLElement>(".bar a, .bar button")]
		.filter((control) => control.checkVisibility())
		.map(
			(control) =>
				control.getAttribute("aria-label") ?? control.textContent?.trim(),
		);

// Kills a bar that names the project (the link's slug may name nothing), a
// phone bar that drops Sign up or leaves Docs and the theme toggle out of its
// menu, and a reader who is asked to log in, or not, by the wrong bar.
test("the bar holds the wordmark, Docs and the theme or the menu, and Log in and Sign up or the avatar", async () => {
	for (const view of ["plot", "locked"] as const) {
		show({ view });
		for (const [width, tools] of [
			[1280, ["Docs", "Switch between light and dark"]],
			[390, ["Menu"]],
			[320, ["Menu"]],
		] as const) {
			await page.viewport(width, 800);
			document.documentElement.removeAttribute("data-reader");
			expect(held(), `${view} at ${width}`).toEqual([
				"Bencher, home",
				...tools,
				"Log in",
				"Sign up",
			]);
			document.documentElement.setAttribute("data-reader", "");
			expect(held(), `${view} at ${width}, signed in`).toEqual([
				"Bencher, home",
				...tools,
				"Account",
			]);
		}
		dispose?.();
		document.body.replaceChildren();
		document.documentElement.removeAttribute("data-reader");
	}
});

// Kills a public wordmark that follows the reader's system rather than the
// page's theme, both drawn at once, or a theme with none.
test("the wordmark follows the page's theme", async () => {
	await page.viewport(1280, 900);
	show();
	const html = document.documentElement;
	const wordmark = () =>
		[...document.querySelectorAll<HTMLImageElement>(".brand img")]
			.filter((image) => image.checkVisibility())
			.map((image) => new URL(image.src).pathname);

	expect(wordmark()).toEqual(["/wordmark-dark.svg"]);
	html.setAttribute("data-theme", "light");
	expect(wordmark()).toEqual(["/wordmark-light.svg"]);
	html.setAttribute("data-theme", "dark");
	expect(wordmark()).toEqual(["/wordmark-dark.svg"]);
});

// Kills a phone bar that wraps to a second row, scrolls sideways, or pushes
// Log in or Sign up off the screen.
test("on a phone the bar keeps Log in and Sign up on one row down to 320 px", async () => {
	show();
	const bar = document.querySelector(".bar");
	for (const width of [390, 320]) {
		await page.viewport(width, 800);
		expect(bar?.getBoundingClientRect().height).toBe(52);
		expect(bar && bar.scrollWidth <= bar.clientWidth).toBe(true);
		for (const name of ["Log in", "Sign up"]) {
			const link = page.getByRole("link", { name }).element();
			const { left, right, height } = link.getBoundingClientRect();
			expect(left, `${name} at ${width}`).toBeGreaterThanOrEqual(0);
			expect(right, `${name} at ${width}`).toBeLessThanOrEqual(width);
			expect(height, `${name} at ${width}`).toBeGreaterThanOrEqual(44);
		}
	}
});
