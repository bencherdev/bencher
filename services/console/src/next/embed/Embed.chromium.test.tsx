import "./page.css";
import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import {
	AlertStatus,
	BoundaryLimit,
	type JsonConsolePerf,
} from "../../types/bencher";
import Plot from "../plot/Plot";
import { publicPlotPath } from "../public/query";
import { type ExploreQuery, blankQuery, decodeQuery } from "../query/query";
import Embed, { type EmbedLinks, type EmbedProps } from "./Embed";

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
		{ uuid: uuid(30), benchmark: 0, parameters: { threads: 1 } },
		{ uuid: uuid(31), benchmark: 0, parameters: { threads: 4 } },
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
		series: { y: [variant + 1], alerts: [] },
	})),
};

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1) }],
	testbeds: [{ uuid: uuid(3) }],
	benchmarks: [uuid(5)],
	measures: [uuid(7)],
	xAxis: "version",
};

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** The embed of hashbrown with `props` over a drawn plot; an undefined prop stays undefined. */
const show = (props: Partial<EmbedProps> = {}) => {
	const shown: EmbedProps = {
		slug: "hashbrown",
		name: "Hashbrown",
		query: QUERY,
		view: "plot",
		answer: PERF,
		plot: Plot,
		...props,
	};
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => {
		const [query, setQuery] = createSignal(shown.query);
		return (
			<Embed
				slug={shown.slug}
				name={shown.name}
				nameless={shown.nameless}
				query={query()}
				view={shown.view}
				answer={shown.answer}
				refused={shown.refused}
				plot={shown.plot}
				onQuery={setQuery}
				links={shown.links}
				wordmarks={shown.wordmarks}
				actions={shown.actions}
			/>
		);
	}, root);
};

const linked = (name: string) => {
	const link = page
		.getByRole("link", { name, exact: true })
		.element() as HTMLAnchorElement;
	const href = new URL(link.href);
	return {
		path: href.pathname,
		query: decodeQuery(href.search),
		target: link.target,
	};
};

// Kills an embed that draws the bar, the query panel, or an action, a line
// whose links open another page or query or leave the frame, and links that
// keep a line the reader hid or miss the one they focused.
test("draws the plot and its key under one line that opens the public plot of this query", async () => {
	await page.viewport(1280, 900);
	show();
	const figure = page.getByRole("figure");
	await expect.element(figure).toHaveAccessibleName("2 lines");
	for (const name of ["Hashbrown", "Bencher"]) {
		expect(linked(name)).toEqual({
			path: publicPlotPath("hashbrown"),
			query: QUERY,
			target: "_blank",
		});
	}
	expect(page.getByRole("banner").elements()).toHaveLength(0);
	expect(page.getByRole("region", { name: /Query/ }).elements()).toHaveLength(
		0,
	);
	const buttons = page.getByRole("button").elements();
	expect(buttons.length).toBeGreaterThan(0);
	for (const button of buttons) {
		expect(figure.element().contains(button)).toBe(true);
	}

	await userEvent.click(
		page.getByRole("button", { name: /hide its line/ }).first(),
	);
	await expect
		.element(page.getByRole("button", { name: /show its line/ }))
		.toBeVisible();
	for (const name of ["Hashbrown", "Bencher"]) {
		expect(linked(name).query.hide).toHaveLength(1);
	}

	await userEvent.click(
		page.getByRole("button", { name: /^Focus line of / }).last(),
	);
	await expect
		.element(page.getByRole("button", { name: /^Clear focus of / }))
		.toBeVisible();
	for (const name of ["Hashbrown", "Bencher"]) {
		expect(linked(name).query.focus).toBeDefined();
	}
});

// Kills a line or plot that moves as the name and the plot arrive, at any width.
test("nothing moves as the name and the plot arrive", async () => {
	for (const width of [1280, 600, 390, 320]) {
		await page.viewport(width, 900);
		const [name, setName] = createSignal<string>();
		const [answer, setAnswer] = createSignal<JsonConsolePerf>();
		const [plot, setPlot] = createSignal<typeof Plot>();
		const root = document.createElement("div");
		document.body.append(root);
		dispose = render(
			() => (
				<Embed
					slug="hashbrown"
					name={name()}
					query={QUERY}
					view="plot"
					answer={answer()}
					plot={plot()}
				/>
			),
			root,
		);
		const place = () => {
			const brand = page.getByRole("link", { name: "Bencher" }).element();
			const figure = page.getByRole("figure").element();
			return {
				brand: brand.getBoundingClientRect().toJSON(),
				figure: figure.getBoundingClientRect().top,
			};
		};
		await expect.element(page.getByRole("figure")).toBeVisible();
		const before = place();
		setName("Hashbrown");
		setPlot(() => Plot);
		setAnswer(PERF);
		await expect
			.element(page.getByRole("figure"))
			.toHaveAccessibleName("2 lines");
		await expect
			.element(page.getByRole("link", { name: "Hashbrown" }))
			.toBeVisible();
		expect(place(), `at ${width}`).toEqual(before);
		expect(document.documentElement.scrollWidth, `at ${width}`).toBe(width);
		dispose();
		dispose = undefined;
		document.body.replaceChildren();
	}
});

// Kills a name that waits for good after its read failed with no answer to lock on.
test("a project whose read failed names it by its slug", async () => {
	show({ name: undefined, nameless: true, answer: undefined });
	expect(linked("hashbrown").path).toBe(publicPlotPath("hashbrown"));
	expect(document.querySelector(".em-name .ui-skeleton")).toBeNull();
});

// Kills a project the API refused drawn as a plot, or an embed that offers an
// action for it.
test("a project the API refused says so, with no plot and no action", async () => {
	show({ name: undefined, view: "locked", answer: undefined });
	await expect.element(page.getByText("Private or not found")).toBeVisible();
	expect(page.getByRole("figure").elements()).toHaveLength(0);
	expect(page.getByRole("button").elements()).toHaveLength(0);
	expect(linked("hashbrown").path).toBe(publicPlotPath("hashbrown"));
});

// Kills a failed plot read drawn as a plot that is still loading, or one that
// offers a retry.
test("a plot that did not load says so, with no action", async () => {
	show({ answer: undefined, refused: "The plot did not load" });
	await expect.element(page.getByText("The plot did not load")).toBeVisible();
	expect(
		page.getByRole("img", { name: "Loading the plot" }).elements(),
	).toHaveLength(0);
	expect(page.getByRole("button").elements()).toHaveLength(0);
});

// Kills a query with no lines drawn as a plot on its way.
test("a query with no lines says so", async () => {
	show({ query: blankQuery(), answer: undefined });
	await expect.element(page.getByText("No lines")).toBeVisible();
	expect(page.getByRole("figure").elements()).toHaveLength(0);
});

// One line whose middle report raised an alert.
const ALERTED: JsonConsolePerf = {
	...PERF,
	total: 1,
	points: { x: [1, 2, 3], report: [0, 1, 2] },
	reports: [20, 21, 22].map((n) => ({ uuid: uuid(n), version: n })),
	variants: [{ uuid: uuid(30), benchmark: 0, parameters: {} }],
	lines: [
		{
			branch: 0,
			testbed: 0,
			benchmark: 0,
			variant: 0,
			measure: 0,
			metric: "value",
			series: {
				y: [1, 3, 1],
				upper: [2, 2, 2],
				alerts: [
					{
						index: 1,
						uuid: uuid(40),
						limit: BoundaryLimit.Upper,
						status: AlertStatus.Active,
					},
				],
			},
		},
	],
};

const LINKS: EmbedLinks = {
	plot: "https://bencher.test/console/projects/hashbrown/explore",
	report: (uuid) => `https://bencher.test/reports/${uuid}`,
	alert: (uuid) => `https://bencher.test/alerts/${uuid}`,
};

const svg = (fill: string) =>
	`data:image/svg+xml,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 1"><rect width="6" height="1" fill="${fill}"/></svg>`)}`;

// Kills a link prop that only one of the line's links opens, a readout that
// keeps no link to the report or the alert it pins, an alert the readout
// offers to change, and wordmarks that still load from the site.
test("links and wordmarks replace the line's and give the readout its links, with no alert action", async () => {
	await page.viewport(1280, 900);
	const wordmarks = { dark: svg("white"), light: svg("black") };
	show({ answer: ALERTED, links: LINKS, wordmarks });
	for (const name of ["Hashbrown", "Bencher"]) {
		const link = page
			.getByRole("link", { name, exact: true })
			.element() as HTMLAnchorElement;
		expect(link.href).toBe(LINKS.plot);
	}
	const images = [...document.querySelectorAll<HTMLImageElement>(".brand img")];
	expect(images.map(({ src }) => src)).toEqual([
		wordmarks.dark,
		wordmarks.light,
	]);

	// As in Explore, the readout offers the alert of the line in focus.
	await userEvent.click(
		page.getByRole("button", { name: /^Focus line of / }).first(),
	);
	const area = page.getByRole("group", { name: /lines?,/ });
	const box = area.element().getBoundingClientRect();
	await area.click({ position: { x: box.width / 2, y: box.height / 2 } });
	const alert = page.getByRole("link", { name: /^Alert: / });
	await expect.element(alert).toHaveAttribute("href", LINKS.alert(uuid(40)));
	const report = document.querySelector<HTMLAnchorElement>(".pl-when a");
	expect(report?.href).toBe(LINKS.report(uuid(21)));
	expect(
		page
			.getByRole("button", { name: /^(Dismiss|Reactivate) the alert/ })
			.elements(),
	).toHaveLength(0);
});

// Kills actions that sit anywhere but the end of the line, or a line that
// grows to hold a phone's full height button and moves the plot down.
test("actions end the line and move nothing, at a phone's width too", async () => {
	for (const width of [1280, 390]) {
		await page.viewport(width, 900);
		const place = () => ({
			line: document
				.querySelector(".em-line")
				?.getBoundingClientRect()
				.toJSON(),
			figure: page.getByRole("figure").element().getBoundingClientRect().top,
		});
		show();
		await expect
			.element(page.getByRole("figure"))
			.toHaveAccessibleName("2 lines");
		const without = place();
		dispose?.();
		document.body.replaceChildren();

		show({
			actions: (
				<Button variant="ghost" iconOnly aria-label="Close">
					<Icon name="close" />
				</Button>
			),
		});
		await expect
			.element(page.getByRole("figure"))
			.toHaveAccessibleName("2 lines");
		const action = page.getByRole("button", { name: "Close" }).element();
		if (width < 768) {
			expect(action.getBoundingClientRect().height).toBeGreaterThan(
				without.line?.height ?? 0,
			);
		}
		expect(action.parentElement?.lastElementChild, `at ${width}`).toBe(action);
		expect(
			action.parentElement?.classList.contains("em-line"),
			`at ${width}`,
		).toBe(true);
		expect(place(), `at ${width}`).toEqual(without);
		dispose?.();
		dispose = undefined;
		document.body.replaceChildren();
	}
});
