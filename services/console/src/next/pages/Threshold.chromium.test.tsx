import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import "../plot/plot.css";
import "../report/report.css";
import "../thresholds/thresholds.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import { type Api, ApiError } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { decodeQuery } from "../query/query";
import { fakeApi } from "../reports/testing";
import { alertsBatch } from "../thresholds/query";
import {
	THRESHOLD,
	alertsFixture,
	alertsPage,
	thresholdFixture,
} from "../thresholds/testing";
import Threshold from "./Threshold";

const PATH = `/hashbrown/thresholds/${THRESHOLD}`;
const EXPLORE = "Open in Explore: the lines this threshold alerted on";
const DAY = 86_400_000;

let dispose: (() => void) | undefined;

const mount = (api: Api, client: QueryClient, path = PATH) => {
	const history = createMemoryHistory();
	history.set({ value: path });
	const root = document.createElement("div");
	root.className = "console";
	root.id = "console";
	root.dataset.apiUrl = "https://api.bencher.dev";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<MemoryRouter
					history={history}
					root={(props) => (
						<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
							<Suspense fallback={<p>Suspended</p>}>{props.children}</Suspense>
						</ProjectContext.Provider>
					)}
				>
					<Route path="/:project/thresholds/:threshold" component={Threshold} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return history;
};

const client = () => createQueryClient(() => {});

/** The API: the threshold, its alerts, and the bootstrap, each answered when `open` says so. */
const api = (
	open: { threshold?: Promise<void>; alerts?: Promise<void> } = {},
) =>
	fakeApi(async (url) => {
		if (url.pathname.endsWith(`/console/thresholds/${THRESHOLD}`)) {
			await open.threshold;
			return { data: thresholdFixture() };
		}
		if (url.pathname.endsWith("/console/alerts")) {
			await open.alerts;
			return { data: alertsFixture() };
		}
		return new Promise<never>(() => {});
	});

const alertsTable = () =>
	page.getByRole("table", { name: "Alerts this threshold raised" });
const alertRows = () => alertsTable().getByRole("button", { name: /^Expand / });
const summary = () =>
	page
		.getByRole("region", { name: "Alerts raised" })
		.getByText(/^\d+ active · /);
const isAlerts = (url: URL) => url.pathname.endsWith("/console/alerts");
const asked = (requests: URL[], batch: number) =>
	requests.filter(
		(url) => isAlerts(url) && url.searchParams.get("page") === String(batch),
	).length;
/** The row numbers drawn, the header's 1 left out. */
const drawn = () =>
	[...document.querySelectorAll<HTMLElement>("tbody tr[aria-rowindex]")].map(
		(row) => Number(row.getAttribute("aria-rowindex")),
	);
const toBottom = () =>
	window.scrollTo({ top: document.body.scrollHeight, behavior: "instant" });
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

beforeEach(async () => {
	await page.viewport(1280, 900);
});

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	window.scrollTo({ top: 0, behavior: "instant" });
});

// Kills alerts asked for only once the threshold answered, which chains a
// second round, and a request that leaves the status to the API's default.
test("a cold threshold asks for itself and its alerts in one round", async () => {
	let release: (() => void) | undefined;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const { api: held_api, requests } = api({ threshold: held, alerts: held });
	mount(held_api, client());

	await expect
		.poll(() =>
			requests
				.map((url) => url.pathname)
				.filter((path) => path.includes("/console/")),
		)
		.toEqual([
			`/v0/projects/hashbrown/console/thresholds/${THRESHOLD}`,
			"/v0/projects/hashbrown/console/alerts",
		]);
	const alerts = requests.find((url) => url.pathname.endsWith("/alerts"));
	expect(alerts?.searchParams.get("thresholds")).toBe(THRESHOLD);
	expect(alerts?.searchParams.get("status")).toBe("all");
	release?.();

	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("main · ubuntu-latest · Latency · value");
	await expect.element(alertRows().first()).toBeVisible();
});

// Kills a model history that drops the replaced model or orders it first, and
// a history row that does not say when each model held.
test("the model history lists every model, newest first, with when each held", async () => {
	const { api: answered } = api();
	mount(answered, client());

	const history = page.getByRole("region", { name: "Model history" });
	await expect
		.element(history.getByRole("listitem").nth(0))
		.toHaveTextContent(
			/^current t_test · upper_boundary 0\.99 · max_sample_size 64 · min_sample_size 4 since Aug 30$/,
		);
	await expect
		.element(history.getByRole("listitem").nth(1))
		.toHaveTextContent(
			/^replaced z_score · upper_boundary 0\.98 · max_sample_size 30 Aug 2 to Aug 30$/,
		);
});

// Kills one line alerting twice drawn as one row, a row that does not name
// its report or its status, and a dismissed alert drawn as alerting.
test("each alert is its own row, naming its report and its status", async () => {
	const { api: answered } = api();
	mount(answered, client());

	await expect.poll(() => alertRows().all().length).toBe(3);
	const rows = page
		.getByRole("table", { name: "Alerts this threshold raised" })
		.getByRole("row");
	await expect
		.element(rows.nth(1).getByRole("link", { name: /^Open the report from / }))
		.toHaveAttribute(
			"href",
			"/next/console/projects/hashbrown/reports/report-new",
		);
	await expect
		.element(rows.nth(1).getByRole("img", { name: "alerting" }))
		.toBeVisible();
	await expect.element(rows.nth(2)).toHaveTextContent(/· dismissed/);
	expect(rows.nth(2).getByRole("img", { name: "alerting" }).query()).toBeNull();
	await expect.element(rows.nth(3)).toHaveTextContent(/· silenced/);
	await expect
		.element(page.getByText("1 active · 2 dismissed in the last 4 weeks"))
		.toBeVisible();
});

// Kills an Explore link offered before there is anything to open, and one
// that is never offered once the alerts arrive.
test("Open in Explore links once the alerts arrive", async () => {
	let release: (() => void) | undefined;
	const { api: held } = api({
		alerts: new Promise<void>((resolve) => {
			release = resolve;
		}),
	});
	mount(held, client());
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("main · ubuntu-latest · Latency · value");
	await expect
		.element(page.getByRole("button", { name: "Open in Explore" }))
		.toBeDisabled();

	release?.();
	const link = page.getByRole("link", { name: EXPLORE });
	await expect.element(link).toBeVisible();
	expect(link.element().getAttribute("href")).toMatch(
		/^\/next\/console\/projects\/hashbrown\/explore\?branches=main-uuid&/,
	);
});

// Kills a status change that never reaches the API or the link.
test("the status toggle asks for its alerts and keeps it in the link", async () => {
	const { api: answered, requests } = api();
	const history = mount(answered, client());
	await expect.element(alertRows().first()).toBeVisible();

	await page.getByRole("radio", { name: "Dismissed" }).click();
	await expect
		.poll(() =>
			requests
				.filter((url) => url.pathname.endsWith("/alerts"))
				.map((url) => url.searchParams.get("status")),
		)
		.toEqual(["all", "dismissed"]);
	expect(history.get()).toBe(`${PATH}?status=dismissed`);
});

// Kills an Explore link that outlives the rows it opens, an empty selection.
test("Open in Explore turns off again for a view with no alerts", async () => {
	const { api: answered } = fakeApi((url) => {
		if (url.pathname.endsWith(`/console/thresholds/${THRESHOLD}`)) {
			return { data: thresholdFixture() };
		}
		if (url.pathname.endsWith("/console/alerts")) {
			return {
				data:
					url.searchParams.get("status") === "dismissed"
						? alertsFixture({ groups: [], total: 0 })
						: alertsFixture(),
			};
		}
		return new Promise<never>(() => {});
	});
	mount(answered, client());
	await expect.element(page.getByRole("link", { name: EXPLORE })).toBeVisible();

	await page.getByRole("radio", { name: "Dismissed" }).click();
	await expect
		.element(page.getByRole("heading", { name: "No alerts in this window" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "Open in Explore" }))
		.toBeDisabled();
});

// Kills a summary that keeps its first answer, and a Window control whose
// pick never reaches the API or the link.
test("the summary follows the window, its counts and its phrase", async () => {
	const { api: answered, requests } = fakeApi((url) => {
		if (url.pathname.endsWith(`/console/thresholds/${THRESHOLD}`)) {
			return { data: thresholdFixture() };
		}
		if (isAlerts(url)) {
			return {
				data: url.searchParams.has("end_time")
					? alertsFixture({
							groups: [],
							total: 0,
							counts: { active: 0, dismissed: 0, silenced: 0 },
						})
					: url.searchParams.get("window") === "7"
						? alertsFixture({
								counts: { active: 1, dismissed: 0, silenced: 0 },
							})
						: alertsFixture(),
			};
		}
		return new Promise<never>(() => {});
	});
	const history = mount(answered, client());
	await expect
		.element(summary())
		.toHaveTextContent("1 active · 2 dismissed in the last 4 weeks");

	await page.getByRole("radio", { name: "1w" }).click();
	await expect
		.element(summary())
		.toHaveTextContent("1 active · 0 dismissed in the last week");
	expect(history.get()).toBe(`${PATH}?window=1w`);
	const week = requests.filter(isAlerts).at(-1);
	expect(week?.searchParams.get("window")).toBe("7");
	expect(
		Math.round(
			(Date.now() - Number(week?.searchParams.get("start_time"))) / DAY,
		),
	).toBe(7);

	await page.getByRole("radio", { name: "Custom" }).click();
	await expect
		.element(summary())
		.toHaveTextContent(
			/^0 active · 0 dismissed from [A-Z][a-z]{2} \d{1,2} to [A-Z][a-z]{2} \d{1,2}$/,
		);
	expect(requests.filter(isAlerts).at(-1)?.searchParams.has("end_time")).toBe(
		true,
	);
});

// Kills a title that keeps its first answer when the threshold answers again.
test("the title follows its threshold when it answers again", async () => {
	let branch = "main";
	const { api: answered } = fakeApi((url) => {
		if (url.pathname.endsWith(`/console/thresholds/${THRESHOLD}`)) {
			return {
				data: thresholdFixture({
					branch: { uuid: "main-uuid", name: branch, slug: branch },
				}),
			};
		}
		return isAlerts(url)
			? { data: alertsFixture() }
			: new Promise<never>(() => {});
	});
	const cache = client();
	mount(answered, cache);
	const title = page.getByRole("heading", { level: 1 });
	await expect
		.element(title)
		.toHaveTextContent("main · ubuntu-latest · Latency · value");

	branch = "trunk";
	await cache.refetchQueries({
		queryKey: ["console", "threshold", "hashbrown", THRESHOLD],
		exact: true,
	});
	await expect
		.element(title)
		.toHaveTextContent("trunk · ubuntu-latest · Latency · value");
});

// Kills Open in Explore over four weeks whatever the page's window.
test("Open in Explore opens the page's window", async () => {
	const { api: answered } = api();
	mount(answered, client(), `${PATH}?window=1w`);
	const link = page.getByRole("link", { name: EXPLORE });
	await expect.element(link).toBeVisible();
	const query = decodeQuery(
		new URL(link.element().getAttribute("href") ?? "", location.origin).search,
	);
	expect(query.window).toEqual({ seconds: 7 * 86_400 });
});

// Kills an archived threshold called active, and an Applies to card that does
// not say which dimension archived it.
test("an archived threshold says so, and what archived it", async () => {
	const { api: answered } = fakeApi((url) => {
		if (url.pathname.endsWith(`/console/thresholds/${THRESHOLD}`)) {
			return {
				data: thresholdFixture({
					testbed: {
						uuid: "testbed-uuid",
						name: "ubuntu-old",
						slug: "ubuntu-old",
						archived: Date.parse("2026-09-08T12:00:00Z"),
					},
				}),
			};
		}
		return isAlerts(url)
			? { data: alertsFixture() }
			: new Promise<never>(() => {});
	});
	mount(answered, client());

	await expect
		.element(page.getByText("archived", { exact: true }))
		.toBeVisible();
	expect(page.getByText("active", { exact: true }).query()).toBeNull();
	await expect
		.element(page.getByRole("region", { name: "Applies to" }))
		.toHaveTextContent(/Archived Sep 8(, 2026)? with testbed ubuntu-old/);
});

// Kills Bencher Cloud's own URL printed as the run's host, and the measure
// named by its name rather than the slug the CLI takes.
test("the run snippet names the measure by slug, with no host on Bencher Cloud", async () => {
	const { api: answered } = api();
	mount(answered, client());
	const change = page.getByRole("region", { name: "Change this threshold" });
	await expect.element(change).toHaveTextContent("--threshold-measure latency");
	expect(change.element().textContent).not.toContain("--host");
});

// Kills alerts that never load past their first batch, or ask again for a
// batch they have.
test("scrolling to the end loads each next batch of alerts once", async () => {
	const perPage = alertsBatch();
	const total = 3 * perPage;
	const { api: answered, requests } = fakeApi((url) => {
		if (url.pathname.endsWith(`/console/thresholds/${THRESHOLD}`)) {
			return { data: thresholdFixture() };
		}
		if (isAlerts(url)) {
			const at = Number(url.searchParams.get("page"));
			return { data: alertsPage((at - 1) * perPage, perPage, total) };
		}
		return new Promise<never>(() => {});
	});
	mount(answered, client());
	await expect.element(alertRows().first()).toBeVisible();

	await expect
		.poll(
			() => {
				toBottom();
				return drawn().at(-1);
			},
			{ timeout: 5_000 },
		)
		.toBe(total + 1);
	await wait(200);
	expect([1, 2, 3, 4].map((batch) => asked(requests, batch))).toEqual([
		1, 1, 1, 0,
	]);
});

// Kills a later batch of alerts the API refused that fails without a word, or
// whose Retry asks for nothing.
test("a later batch of alerts the API refused offers Retry, which loads it", async () => {
	const perPage = alertsBatch();
	let refuse = true;
	const { api: answered, requests } = fakeApi((url) => {
		if (url.pathname.endsWith(`/console/thresholds/${THRESHOLD}`)) {
			return { data: thresholdFixture() };
		}
		if (isAlerts(url)) {
			const at = Number(url.searchParams.get("page"));
			if (at === 2 && refuse) {
				return Promise.reject(new ApiError(429, "client", "Too many requests"));
			}
			return { data: alertsPage((at - 1) * perPage, perPage, 2 * perPage) };
		}
		return new Promise<never>(() => {});
	});
	mount(answered, client());
	await expect.element(alertRows().first()).toBeVisible();

	const failed = alertsTable().getByRole("alert");
	await expect
		.poll(
			() => {
				toBottom();
				return failed.query() !== null;
			},
			{ timeout: 5_000 },
		)
		.toBe(true);
	await wait(300);
	expect(asked(requests, 2)).toBe(1);

	refuse = false;
	await failed.getByRole("button", { name: "Retry" }).click();
	await expect
		.poll(
			() => {
				toBottom();
				return drawn().at(-1);
			},
			{ timeout: 5_000 },
		)
		.toBe(2 * perPage + 1);
	expect(asked(requests, 2)).toBe(2);
});

// Kills the last threshold's alerts drawn as the next one's while its own load.
test("moving to another threshold never shows the last one's alerts as its own", async () => {
	const OTHER = "00000000-0000-4000-8000-0000000000b2";
	let release: (() => void) | undefined;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const { api: answered } = fakeApi(async (url) => {
		if (url.pathname.endsWith(`/console/thresholds/${THRESHOLD}`)) {
			return { data: thresholdFixture() };
		}
		if (url.pathname.endsWith(`/console/thresholds/${OTHER}`)) {
			return {
				data: thresholdFixture({
					uuid: OTHER,
					branch: { uuid: "trunk-uuid", name: "trunk", slug: "trunk" },
				}),
			};
		}
		if (isAlerts(url)) {
			if (url.searchParams.get("thresholds") === OTHER) {
				await held;
				return { data: alertsFixture({ groups: [], total: 0 }) };
			}
			return { data: alertsFixture() };
		}
		return new Promise<never>(() => {});
	});
	const history = mount(answered, client());
	await expect.element(alertRows().first()).toBeVisible();

	history.set({ value: `/hashbrown/thresholds/${OTHER}` });
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("trunk · ubuntu-latest · Latency · value");
	await expect
		.element(page.getByText("Loading the alerts"))
		.toBeInTheDocument();
	expect(alertRows().all()).toHaveLength(0);

	release?.();
	await expect
		.element(page.getByRole("heading", { name: "No alerts in this window" }))
		.toBeVisible();
});

// Kills a Status control that keeps its own width on a phone.
test("on a phone, Status spans the page", async () => {
	await page.viewport(390, 844);
	const { api: answered } = api();
	mount(answered, client());
	const status = page.getByRole("radiogroup", { name: "Status" });
	await expect.element(status).toBeVisible();
	expect(status.element().getBoundingClientRect().width).toBeGreaterThan(300);
});
