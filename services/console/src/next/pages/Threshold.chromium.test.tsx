import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import "../plot/plot.css";
import "../report/report.css";
import "../thresholds/thresholds.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import {
	type QueryClient,
	QueryClientProvider,
	focusManager,
} from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonConsoleAlerts,
	JsonConsoleProject,
} from "../../types/bencher";
import { type Api, ApiError } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { decodeQuery } from "../query/query";
import { fakeApi } from "../reports/testing";
import { expectNameStartsWithText } from "../ui/testing";
import { HEIGHTS } from "../report/LineTable";
import { alertsBatch } from "../thresholds/query";
import {
	THRESHOLD,
	alertsFixture,
	alertsPage,
	namedAlerts,
	thresholdFixture,
} from "../thresholds/testing";
import Threshold from "./Threshold";
import { consoleProjectQuery } from "../queries";
import { bootstrapOf } from "../settings/testing";

const PATH = `/hashbrown/thresholds/${THRESHOLD}`;
const EXPLORE = "Open in Explore: the lines this threshold alerted on";
const DAY = 86_400_000;

let dispose: (() => void) | undefined;

const mount = (
	api: Api,
	client: QueryClient,
	path = PATH,
	host = "https://api.bencher.dev",
) => {
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
						<ProjectContext.Provider
							value={{ api, slug: () => "hashbrown", host }}
						>
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
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
			await open.threshold;
			return { data: thresholdFixture() };
		}
		if (url.pathname.endsWith("/v0/console/projects/hashbrown/alerts")) {
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
const isAlerts = (url: URL) =>
	url.pathname.endsWith("/v0/console/projects/hashbrown/alerts");
/** How many times the alerts were asked for from `offset`. */
const asked = (requests: URL[], offset: number) =>
	requests.filter(
		(url) => isAlerts(url) && url.searchParams.get("offset") === String(offset),
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
				.filter((path) => path.includes("/v0/console/projects/hashbrown/")),
		)
		.toEqual([
			`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			"/v0/console/projects/hashbrown/alerts",
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

// Kills a dimension named in plain text, or linked to a page by its name
// rather than its slug.
test("Applies to links the branch, the testbed, and the measure to their pages", async () => {
	const { api: answered } = api();
	mount(answered, client());
	const applies = page.getByRole("region", { name: "Applies to" });
	for (const [name, path] of [
		["main", "branches/main"],
		["ubuntu-latest", "testbeds/ubuntu-latest"],
		["Latency", "measures/latency"],
	] as const) {
		await expect
			.element(applies.getByRole("link", { name, exact: true }))
			.toHaveAttribute("href", `/next/console/projects/hashbrown/${path}`);
	}
});

/** The page's API with the reader's bootstrap held, its changes answered by `answer` and recorded. */
const changing = (
	answer: () => Promise<{ changed: number }>,
	permissions: { edit?: boolean } = {},
) => {
	const { api: answered } = api();
	const sent: { method: string; path: string; body: unknown }[] = [];
	answered.send = async <T,>(method: string, path: string, body?: unknown) => {
		sent.push({ method, path, body });
		return { data: (await answer()) as T, headers: new Headers() };
	};
	const cache = client();
	cache.setQueryData(consoleProjectQuery(answered, "hashbrown").queryKey, {
		...bootstrapOf(permissions),
		active_alerts: 1,
	});
	return { api: answered, cache, sent };
};
const badge = (cache: QueryClient) =>
	cache.getQueryData<JsonConsoleProject>(["console", "project", "hashbrown"])
		?.active_alerts;
const dismiss = () =>
	alertsTable().getByRole("button", { name: /^Dismiss the alert on blake3/ });
const activeRow = () => dismiss().element().closest("tr");

// Kills a threshold page that names a dismissed alert's status in its report
// cell but cannot change it, unlike the Alerts page's rows: Dismiss greys the
// row in place, says so with Reactivate, sends one change, and moves the
// counts and the tab's badge. Also kills rows drawn anew once the API's answer
// is written, before they are handed the rows drawn so far.
test("Dismiss greys the alert's row in place with Reactivate, as the Alerts page does", async () => {
	const { api: answered, cache, sent } = changing(async () => ({ changed: 1 }));
	mount(answered, cache);
	await expect.element(dismiss()).toBeVisible();
	const row = activeRow();
	await dismiss().click();
	await expect.element(summary()).toHaveTextContent(/^0 active · 3 dismissed/);
	const reactivate = alertsTable().getByRole("button", {
		name: /^Reactivate the alert on blake3/,
	});
	await expect.element(reactivate).toBeVisible();
	await wait(50);
	expect(reactivate.element().closest("tr")).toBe(row);
	expect(row?.textContent).toContain("Dismissed just now");
	expect(row?.classList.contains("lr-dim")).toBe(true);
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/console/projects/hashbrown/alerts",
			body: { status: "dismissed", alerts: ["alert-1"] },
		},
	]);
	expect(badge(cache)).toBe(0);
});

// Kills a refusal that leaves the row grey, or the counts and the badge moved,
// and one that says nothing.
test("a refused dismiss puts the row, the counts, and the badge back, and says so", async () => {
	const { api: answered, cache } = changing(async () => {
		throw new ApiError(500, "server", "{}");
	});
	mount(answered, cache);
	await expect.element(dismiss()).toBeVisible();
	await dismiss().click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("The alert did not change");
	await expect.element(dismiss()).toBeVisible();
	await expect.element(summary()).toHaveTextContent(/^1 active · 2 dismissed/);
	expect(badge(cache)).toBe(1);
});

// Kills status controls drawn for a reader who cannot change an alert.
test("a reader who cannot edit sees each alert's status and no Dismiss or Reactivate", async () => {
	const { api: answered, cache } = changing(async () => ({ changed: 1 }), {
		edit: false,
	});
	mount(answered, cache);
	await expect.poll(() => alertRows().all().length).toBe(3);
	await expect.element(alertsTable()).toHaveTextContent(/Dismissed Aug 27/);
	expect(dismiss().query()).toBeNull();
	expect(
		alertsTable()
			.getByRole("button", { name: /^Reactivate / })
			.query(),
	).toBeNull();
});

/**
 * The page's API with its alerts answered by `alerts` (each read numbered from
 * 0) and its changes by `answer`, for a reader who can edit.
 */
const editing = (
	alerts: (url: URL, read: number) => JsonConsoleAlerts,
	answer: () => Promise<{ changed: number }> = async () => ({ changed: 1 }),
) => {
	let reads = 0;
	const { api: answered, requests } = fakeApi((url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
			return { data: thresholdFixture() };
		}
		if (isAlerts(url)) {
			return { data: alerts(url, reads++) };
		}
		return new Promise<never>(() => {});
	});
	answered.send = async <T,>() => ({
		data: (await answer()) as T,
		headers: new Headers(),
	});
	const cache = client();
	cache.setQueryData(consoleProjectQuery(answered, "hashbrown").queryKey, {
		...bootstrapOf(),
		active_alerts: 5,
	});
	return { api: answered, cache, requests, reads: () => reads };
};
const focusedName = () => document.activeElement?.getAttribute("aria-label");
/** The reader leaves the window and comes back, which reads stale views again. */
const refocus = () => {
	focusManager.setFocused(false);
	focusManager.setFocused(true);
};

// Kills counts moved only in the cached batch that holds the alert: the
// summary reads the view's first batch, so its count fell back once the API
// took a change to an alert in a later batch.
test("a change to an alert in a later batch keeps the summary's counts once the API takes it", async () => {
	const perPage = alertsBatch();
	let release = () => {};
	const { api: answered, cache } = editing(
		(url) => {
			const offset = Number(url.searchParams.get("offset"));
			return alertsPage(offset, perPage, 2 * perPage);
		},
		() =>
			new Promise((resolve) => {
				release = () => resolve({ changed: 1 });
			}),
	);
	mount(answered, cache);
	await expect.element(alertRows().first()).toBeVisible();
	await expect
		.poll(
			() => {
				toBottom();
				return drawn().at(-1);
			},
			{ timeout: 5_000 },
		)
		.toBe(2 * perPage + 1);
	await expect.element(summary()).toHaveTextContent(/^1 active · 2 dismissed/);
	const last = alertsTable()
		.getByRole("button", { name: /^Dismiss the alert on/ })
		.last();
	expect(
		Number(last.element().closest("tr")?.getAttribute("aria-rowindex")),
	).toBeGreaterThan(perPage + 1);
	await last.click();
	await expect.element(summary()).toHaveTextContent(/^0 active · 3 dismissed/);
	release();
	await wait(100);
	await expect.element(summary()).toHaveTextContent(/^0 active · 3 dismissed/);
});

// Kills a row dismissed under Active that leaves when the list is read again
// and drops the focus with it: the Alerts page keeps such a row, grey where it
// was, until the reader leaves.
test("under Active, a dismissed row keeps its place and the focus through a reread", async () => {
	const {
		api: answered,
		cache,
		reads,
	} = editing((_, read) =>
		read === 0 ? alertsPage(0, 3, 3) : alertsPage(1, 2, 2),
	);
	mount(answered, cache, `${PATH}?status=active`);
	const first = alertsTable()
		.getByRole("button", { name: /^Dismiss the alert on/ })
		.first();
	await expect.element(first).toBeVisible();
	(first.element() as HTMLElement).focus();
	await userEvent.keyboard("{Enter}");
	await expect.poll(focusedName).toMatch(/^Reactivate the alert on/);
	refocus();
	try {
		await expect.poll(reads).toBe(2);
		await wait(100);
		expect(alertRows().all().length).toBe(3);
		expect(
			alertsTable()
				.getByRole("row")
				.nth(1)
				.getByRole("button", { name: /^Reactivate the alert on/ })
				.query(),
		).not.toBeNull();
		expect(focusedName()).toMatch(/^Reactivate the alert on/);
	} finally {
		focusManager.setFocused(undefined);
	}
});

// Kills a row drawn anew when the list is read again after a change and
// answers the alert with the status the page gave it, which drops the focus
// the row's Dismiss handed to its Reactivate.
test("under All, a reread that answers a dismissed row as the page drew it keeps the focus", async () => {
	const {
		api: answered,
		cache,
		reads,
	} = editing((_, read) => {
		const batch = alertsPage(0, 3, 3);
		const group = batch.groups[0];
		const entry = group?.alerts[0];
		if (read > 0 && group && entry?.line.alert) {
			group.alerts[0] = {
				modified: entry.modified + 5_000,
				line: {
					...entry.line,
					alert: { ...entry.line.alert, status: "dismissed" },
				},
			} as typeof entry;
		}
		return batch;
	});
	mount(answered, cache);
	const first = alertsTable()
		.getByRole("button", { name: /^Dismiss the alert on/ })
		.first();
	await expect.element(first).toBeVisible();
	(first.element() as HTMLElement).focus();
	await userEvent.keyboard("{Enter}");
	await expect.poll(focusedName).toMatch(/^Reactivate the alert on/);
	await wait(100);
	const focused = document.activeElement;
	refocus();
	try {
		await expect.poll(reads).toBe(2);
		await wait(100);
		expect(document.activeElement).toBe(focused);
	} finally {
		focusManager.setFocused(undefined);
	}
});

// Kills a row reactivated under Dismissed that stays, where the Alerts page
// lets it go, and the focus dropped to the page with it rather than handed to
// the row now in its place.
test("under Dismissed, a reactivated row leaves and the focus moves to the row now in its place", async () => {
	const { api: answered, cache } = editing(() => alertsFixture());
	mount(answered, cache, `${PATH}?status=dismissed`);
	const reactivate = alertsTable().getByRole("button", {
		name: /^Reactivate the alert on sha256/,
	});
	await expect.element(reactivate).toBeVisible();
	(reactivate.element() as HTMLElement).focus();
	await userEvent.keyboard("{Enter}");
	await expect.poll(() => alertRows().all().length).toBe(1);
	await expect.poll(focusedName).toMatch(/^Expand blake3/);
});

// Kills a change the API made to no alert that blames the API for not
// answering and reads nothing again, where the Alerts page says so, reads the
// list again, and counts the project's active alerts again.
test("a change the API made to no alert says so, reads the list again, and counts again", async () => {
	const {
		api: answered,
		cache,
		requests,
	} = editing(
		() => alertsFixture(),
		async () => ({ changed: 0 }),
	);
	mount(answered, cache);
	await expect.element(dismiss()).toBeVisible();
	await wait(100);
	const reads = () => requests.filter(isAlerts).length;
	const counts = () =>
		requests.filter((url) =>
			url.pathname.endsWith("/v0/console/projects/hashbrown"),
		).length;
	const before = { reads: reads(), counts: counts() };
	await dismiss().click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Some alerts did not change");
	await expect.poll(reads).toBe(before.reads + 1);
	await expect.poll(counts).toBe(before.counts + 1);
	await expect.element(dismiss()).toBeVisible();
	expect(badge(cache)).toBe(5);
});

const reactivateSha256 = () =>
	alertsTable().getByRole("button", {
		name: /^Reactivate the alert on sha256/,
	});
/** On a phone the alerts sit below the cards, past the rows the list draws, until the reader scrolls to them. */
const scrollToAlerts = async () => {
	await expect.element(alertsTable()).toBeInTheDocument();
	alertsTable()
		.element()
		.scrollIntoView({ block: "start", behavior: "instant" });
	await new Promise((resolve) =>
		requestAnimationFrame(() => requestAnimationFrame(resolve)),
	);
};

for (const width of [900, 390, 360]) {
	// Kills folded rows that give an editor no Dismiss, a folded Dismiss or
	// Reactivate under 44 px square or cut off by its cell (at 360 px, beside a
	// status that does not give way), and a folded row drawn at another height
	// than the list lays it out by.
	test(`at ${width} px a folded row keeps its Dismiss or Reactivate whole, 44 px square`, async () => {
		await page.viewport(width, 900);
		const { api: answered, cache } = changing(async () => ({ changed: 1 }));
		mount(answered, cache);
		await scrollToAlerts();
		await expect.element(dismiss()).toBeVisible();
		await expect.element(reactivateSha256()).toBeVisible();
		expect(alertsTable().element().classList.contains("lr-narrow")).toBe(true);
		for (const control of [dismiss(), reactivateSha256()]) {
			const element = control.element() as HTMLElement;
			const box = element.getBoundingClientRect();
			const cell = element.closest("td")?.getBoundingClientRect();
			const row = element.closest("tr")?.getBoundingClientRect();
			expect(box.width).toBeGreaterThanOrEqual(44);
			expect(box.height).toBeGreaterThanOrEqual(44);
			expect(box.left).toBeGreaterThanOrEqual(cell?.left ?? Number.NaN);
			expect(box.right).toBeLessThanOrEqual(cell?.right ?? Number.NaN);
			expect(box.top).toBeGreaterThanOrEqual(cell?.top ?? Number.NaN);
			expect(box.bottom).toBeLessThanOrEqual(cell?.bottom ?? Number.NaN);
			expect(element.scrollWidth).toBeLessThanOrEqual(element.clientWidth);
			expect(Math.round(row?.height ?? 0)).toBe(
				HEIGHTS.narrow.line + HEIGHTS.narrow.actions,
			);
		}
	});
}

// Kills a folded row without a checkbox that keeps the checkbox's empty
// column, or a dot before its Reactivate, either of which cuts its status short
// on a phone.
test("at 390 px a folded dismissed row reads its status in full beside Reactivate", async () => {
	await page.viewport(390, 900);
	const { api: answered, cache } = changing(async () => ({ changed: 1 }));
	mount(answered, cache);
	await scrollToAlerts();
	await expect.element(reactivateSha256()).toBeVisible();
	const status = reactivateSha256()
		.element()
		.closest("tr")
		?.querySelector(".lr-state") as HTMLElement;
	expect(status.textContent).toBe("Dismissed Aug 27");
	expect(status.scrollWidth).toBeLessThanOrEqual(status.clientWidth);
});

for (const width of [900, 390]) {
	// Kills a folded Dismiss that drops the focus, or hands it anywhere but its
	// row's Reactivate as a wide row does, and one that does not grey the row
	// in place.
	test(`at ${width} px a folded Dismiss greys its row in place and hands the focus to its Reactivate`, async () => {
		await page.viewport(width, 900);
		const {
			api: answered,
			cache,
			sent,
		} = changing(async () => ({
			changed: 1,
		}));
		mount(answered, cache);
		await scrollToAlerts();
		await expect.element(dismiss()).toBeVisible();
		const row = activeRow();
		(dismiss().element() as HTMLElement).focus();
		await userEvent.keyboard("{Enter}");
		await expect.poll(focusedName).toMatch(/^Reactivate the alert on blake3/);
		await expect
			.element(summary())
			.toHaveTextContent(/^0 active · 3 dismissed/);
		await wait(50);
		expect(focusedName()).toMatch(/^Reactivate the alert on blake3/);
		expect(document.activeElement?.closest("tr")).toBe(row);
		expect(row?.classList.contains("lr-dim")).toBe(true);
		expect(sent).toHaveLength(1);
		expect(badge(cache)).toBe(0);
	});

	// Kills a refused folded dismiss that leaves the row grey, or drops the
	// focus rather than handing it back to the row's Dismiss.
	test(`at ${width} px a refused folded dismiss puts the row back and the focus on its Dismiss`, async () => {
		await page.viewport(width, 900);
		const { api: answered, cache } = changing(async () => {
			throw new ApiError(500, "server", "{}");
		});
		mount(answered, cache);
		await scrollToAlerts();
		await expect.element(dismiss()).toBeVisible();
		(dismiss().element() as HTMLElement).focus();
		await userEvent.keyboard("{Enter}");
		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("The alert did not change");
		await expect.poll(focusedName).toMatch(/^Dismiss the alert on blake3/);
		await expect
			.element(summary())
			.toHaveTextContent(/^1 active · 2 dismissed/);
		expect(badge(cache)).toBe(1);
	});

	// Kills a folded row that leaves and hands the focus to the next row's
	// first control, its report link, where a wide row hands it to the expand
	// control of the row now in its place.
	test(`at ${width} px under Dismissed, a folded reactivated row leaves and the focus moves to the expand control of the row now in its place`, async () => {
		await page.viewport(width, 900);
		const { api: answered, cache } = editing(() => alertsFixture());
		mount(answered, cache, `${PATH}?status=dismissed`);
		await scrollToAlerts();
		await expect.element(reactivateSha256()).toBeVisible();
		(reactivateSha256().element() as HTMLElement).focus();
		await userEvent.keyboard("{Enter}");
		await expect.poll(() => alertRows().all().length).toBe(1);
		await expect.poll(focusedName).toMatch(/^Expand blake3/);
	});
}

/** One dismissed alert in one report: the Dismissed view's only row. */
const oneDismissed = (): JsonConsoleAlerts => {
	const all = alertsFixture();
	const [, older] = all.groups;
	return {
		...all,
		total: 1,
		counts: { active: 1, dismissed: 1, silenced: 0 },
		groups: older
			? [{ ...older, total: 1, alerts: older.alerts.slice(0, 1) }]
			: [],
	};
};

for (const width of [1280, 900]) {
	// Kills the focus dropped to the document when the last row leaves the
	// Dismissed view, where no row is left to hand it to.
	test(`at ${width} px under Dismissed, when the last row leaves, the focus moves to the Status control`, async () => {
		await page.viewport(width, 900);
		const { api: answered, cache } = editing(() => oneDismissed());
		mount(answered, cache, `${PATH}?status=dismissed`);
		await scrollToAlerts();
		await expect.element(reactivateSha256()).toBeVisible();
		(reactivateSha256().element() as HTMLElement).focus();
		await userEvent.keyboard("{Enter}");
		await expect.poll(() => alertRows().all().length).toBe(0);
		await expect
			.poll(() => document.activeElement)
			.toBe(
				page
					.getByRole("radiogroup", { name: "Status" })
					.getByRole("radio", { name: "Dismissed" })
					.element(),
			);
	});
}

// Kills an empty line of controls drawn under a folded row whose alert the
// reader cannot change.
test("at 900 px a reader who cannot edit gets no line of controls under an active folded row", async () => {
	await page.viewport(900, 900);
	const { api: answered, cache } = changing(async () => ({ changed: 1 }), {
		edit: false,
	});
	mount(answered, cache);
	const expand = alertRows().first();
	await expect.element(expand).toHaveAccessibleName(/^Expand blake3/);
	await expect.element(alertsTable()).toHaveTextContent(/Dismissed Aug 27/);
	expect(
		Math.round(
			expand.element().closest("tr")?.getBoundingClientRect().height ?? 0,
		),
	).toBe(HEIGHTS.narrow.line);
});

/**
 * The page's API over the alerts numbered 0 to `total - 1`, each active until
 * the page dismisses it, the list served from its `offset`, or from its `page`
 * for a page that asks by page number. `hold` keeps each change out until it
 * resolves, or with `answerLate` only its answer, the API having taken it;
 * `holdList` keeps a list from `offset` out until it resolves, which reads the
 * alerts then. A change counts only the alerts it moved, `elsewhere`
 * dismisses an alert as another reader would, and `arrive` has a newer report
 * raise alerts numbered from `total`, listed ahead of the rest.
 */
const dismissible = (
	total: number,
	hold?: () => Promise<void>,
	{
		holdList,
		answerLate = false,
	}: {
		holdList?: (offset: number) => Promise<void> | undefined;
		answerLate?: boolean;
	} = {},
) => {
	const dismissed = new Set<number>();
	const arrived: number[] = [];
	let changes = 0;
	const { api: listed, requests } = fakeApi(async (url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
			return { data: thresholdFixture() };
		}
		if (isAlerts(url)) {
			const perPage = Number(url.searchParams.get("per_page"));
			const page = url.searchParams.get("page");
			const offset =
				page === null
					? Number(url.searchParams.get("offset"))
					: (Number(page) - 1) * perPage;
			await holdList?.(offset);
			const ids = [
				...arrived,
				...Array.from({ length: total }, (_, id) => id),
			].filter(
				(id) =>
					url.searchParams.get("status") !== "active" || !dismissed.has(id),
			);
			return {
				data: namedAlerts(ids.slice(offset, offset + perPage), ids.length),
			};
		}
		return new Promise<never>(() => {});
	});
	listed.send = async <T,>(_method: string, _path: string, body?: unknown) => {
		const { status, alerts } = body as { status: string; alerts: string[] };
		const ids = alerts.map((uuid) => Number(uuid.replace("alert-", "")));
		if (!answerLate) {
			await hold?.();
		}
		let changed = 0;
		for (const id of ids) {
			const was = dismissed.has(id);
			if (status === "active") {
				dismissed.delete(id);
			} else {
				dismissed.add(id);
			}
			changed += was === dismissed.has(id) ? 0 : 1;
		}
		changes += 1;
		if (answerLate) {
			await hold?.();
		}
		return { data: { changed } as T, headers: new Headers() };
	};
	const cache = client();
	cache.setQueryData(consoleProjectQuery(listed, "hashbrown").queryKey, {
		...bootstrapOf(),
		active_alerts: total,
	});
	return {
		api: listed,
		cache,
		lists: () => requests.filter(isAlerts),
		/** How many changes the API took. */
		landed: () => changes,
		elsewhere: (id: number) => dismissed.add(id),
		arrive: (...ids: number[]) => arrived.unshift(...ids),
	};
};
const dismissBench = (id: number) =>
	alertsTable().getByRole("button", {
		name: `Dismiss the alert on bench-${id} Latency`,
	});
/** The rows' names, top to bottom, read a screen at a time. */
const namesTopToBottom = async () => {
	const names = new Map<number, string>();
	const frame = () =>
		new Promise((resolve) =>
			requestAnimationFrame(() => requestAnimationFrame(resolve)),
		);
	for (
		let top = 0;
		top <= document.documentElement.scrollHeight;
		top += window.innerHeight / 2
	) {
		window.scrollTo({ top, behavior: "instant" });
		await frame();
		for (const row of alertsTable()
			.element()
			.querySelectorAll("tbody tr[aria-rowindex]")) {
			const name = row
				.querySelector(".lr-twist")
				?.getAttribute("aria-label")
				?.replace(/^(Expand|Collapse) /, "");
			if (name) {
				names.set(Number(row.getAttribute("aria-rowindex")), name);
			}
		}
	}
	return [...names]
		.sort(([a], [b]) => a - b)
		.map(([index, name]) => `${index} ${name}`);
};

// Kills a next batch of a threshold's alerts asked for by page number, which
// skips as many alerts as the page dismissed under Active, and one that counts
// the dismissed rows as still in the view, which skips them too.
test("under Active, alerts dismissed before the next batch leave no alert skipped or repeated", async () => {
	const perPage = alertsBatch();
	const total = 2 * perPage + 4;
	const { api: answered, cache, lists, landed } = dismissible(total);
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	await dismissBench(0).click();
	await dismissBench(1).click();
	await expect.poll(landed).toBe(2);

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
	expect(lists().map((url) => url.searchParams.get("offset"))).toEqual([
		"0",
		String(perPage - 2),
		String(2 * perPage - 2),
	]);
	expect(lists().some((url) => url.searchParams.has("page"))).toBe(false);
	expect(await namesTopToBottom()).toEqual(
		Array.from({ length: total }, (_, id) => `${id + 2} bench-${id} Latency`),
	);
});

// Kills a next batch asked for while a change is out, whose offset still
// counts the row that change is taking out of the view, and one that is never
// asked once the change lands.
test("the next batch of a threshold's alerts waits for a change in flight", async () => {
	const perPage = alertsBatch();
	let release = () => {};
	const {
		api: answered,
		cache,
		lists,
	} = dismissible(
		2 * perPage + 4,
		() =>
			new Promise<void>((resolve) => {
				release = resolve;
			}),
	);
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	await dismissBench(0).click();
	toBottom();
	for (let frames = 0; frames < 3; frames++) {
		await new Promise((resolve) => requestAnimationFrame(resolve));
	}
	await wait(100);
	expect(lists()).toHaveLength(1);
	release();
	await expect.poll(() => lists().length).toBe(2);
	expect(lists()[1]?.searchParams.get("offset")).toBe(String(perPage - 1));
});

/** The alerts numbered `ids` once each, top to bottom, those the page dismissed in their place. */
const drawnOnce = async (ids: number[]) => {
	await expect
		.poll(
			() => {
				toBottom();
				return drawn().at(-1);
			},
			{ timeout: 5_000 },
		)
		.toBe(ids.length + 1);
	await wait(200);
	expect(await namesTopToBottom()).toEqual(
		ids.map((id, index) => `${index + 2} bench-${id} Latency`),
	);
};
/** Every alert once, top to bottom, the first one dismissed in its place. */
const everyAlertOnce = (total: number) =>
	drawnOnce(Array.from({ length: total }, (_, id) => id));

// Kills a next batch the API read after a dismissal it took while that batch
// was out, which starts an alert late and is never asked for again from the
// recounted offset, so that alert is never drawn.
test("under Active, a dismissal the API takes while the next batch is out leaves no alert skipped or repeated", async () => {
	const perPage = alertsBatch();
	const total = 2 * perPage + 4;
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const {
		api: answered,
		cache,
		lists,
		landed,
	} = dismissible(total, undefined, {
		holdList: (offset) => (offset > 0 ? held : undefined),
	});
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	toBottom();
	await expect.poll(() => lists().length).toBe(2);
	window.scrollTo({ top: 0, behavior: "instant" });
	await dismissBench(0).click();
	await expect.poll(landed).toBe(1);
	release();
	// Asked for again with the reader still at the top, where nothing nears the end.
	await expect.poll(() => lists().length).toBe(3);

	await everyAlertOnce(total);
	expect(lists().map((url) => url.searchParams.get("offset"))).toEqual([
		"0",
		String(perPage),
		String(perPage - 1),
		String(2 * perPage - 1),
	]);
});

// Kills a next batch that overtook a dismissal and answered while that
// dismissal's own answer was out, which is drawn, or asked for again before
// the dismissal lands and so from an offset that still counts its row.
test("under Active, a next batch that answers before a dismissal it overtook is asked for again once the dismissal lands", async () => {
	const perPage = alertsBatch();
	const total = 2 * perPage + 4;
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	let answer = () => {};
	const {
		api: answered,
		cache,
		lists,
		landed,
	} = dismissible(
		total,
		() =>
			new Promise<void>((resolve) => {
				answer = resolve;
			}),
		{
			holdList: (offset) => (offset > 0 ? held : undefined),
			answerLate: true,
		},
	);
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	toBottom();
	await expect.poll(() => lists().length).toBe(2);
	window.scrollTo({ top: 0, behavior: "instant" });
	await dismissBench(0).click();
	await expect.poll(landed).toBe(1);
	release();
	await wait(100);
	expect(lists()).toHaveLength(2);
	// Still a batch on its way below the rows, not an empty end.
	expect(
		alertsTable().element().querySelector("tr.lr-more .ui-skeleton"),
	).not.toBeNull();
	answer();
	await expect.poll(() => lists().length).toBe(3);
	expect(lists()[2]?.searchParams.get("offset")).toBe(String(perPage - 1));

	await everyAlertOnce(total);
});

// Kills a later batch asked for again, by Retry, while a dismissal is out,
// whose answer the API read after taking that dismissal, which is drawn from
// its old offset rather than asked for again once the dismissal lands.
test("under Active, a later batch retried while a dismissal is out is asked for again once it lands", async () => {
	const perPage = alertsBatch();
	const total = 2 * perPage + 4;
	let refuse = true;
	let answer = () => {};
	const {
		api: answered,
		cache,
		lists,
		landed,
	} = dismissible(
		total,
		() =>
			new Promise<void>((resolve) => {
				answer = resolve;
			}),
		{
			holdList: (offset) =>
				offset > 0 && refuse
					? Promise.reject(new ApiError(429, "client", "Too many requests"))
					: undefined,
			answerLate: true,
		},
	);
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
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
	window.scrollTo({ top: 0, behavior: "instant" });
	await dismissBench(0).click();
	await expect.poll(landed).toBe(1);
	refuse = false;
	await failed.getByRole("button", { name: "Retry" }).click();
	await expect.poll(() => lists().length).toBe(3);
	await wait(100);
	answer();
	await expect.poll(() => lists().length).toBe(4);
	expect(lists()[3]?.searchParams.get("offset")).toBe(String(perPage - 1));

	await everyAlertOnce(total);
});

// Kills a later batch retried after a dismissal landed, read from the offset
// counted before that dismissal, which starts an alert late: its answer is
// set aside and the batch asked for again from the recounted offset.
test("under Active, a later batch retried after a dismissal landed is asked for again from the recounted offset", async () => {
	const perPage = alertsBatch();
	const total = 2 * perPage + 4;
	let refuse = true;
	const {
		api: answered,
		cache,
		lists,
		landed,
	} = dismissible(total, undefined, {
		holdList: (offset) =>
			offset > 0 && refuse
				? Promise.reject(new ApiError(429, "client", "Too many requests"))
				: undefined,
	});
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
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
	window.scrollTo({ top: 0, behavior: "instant" });
	await dismissBench(0).click();
	await expect.poll(landed).toBe(1);
	await wait(100);
	refuse = false;
	await failed.getByRole("button", { name: "Retry" }).click();
	await expect.poll(() => lists().length).toBe(4);
	expect(lists()[3]?.searchParams.get("offset")).toBe(String(perPage - 1));

	await everyAlertOnce(total);
});

/** A gate that holds what awaits it until `open` is called. */
const gate = () => {
	let open = () => {};
	const shut = new Promise<void>((resolve) => {
		open = resolve;
	});
	return { shut, open };
};

// Kills a batch asked for again under the key of an overtaken batch from the
// same offset, which draws that batch's answer, an alert short, until the
// list is read again.
test("under Active, a batch asked again from an overtaken batch's offset never draws that batch's answer", async () => {
	const perPage = alertsBatch();
	const total = 2 * perPage + 4;
	const reads = [gate(), gate(), gate()];
	let later = 0;
	const answers: (() => void)[] = [];
	const {
		api: answered,
		cache,
		lists,
		landed,
	} = dismissible(
		total,
		() =>
			new Promise<void>((resolve) => {
				answers.push(resolve);
			}),
		{
			holdList: (offset) => (offset > 0 ? reads[later++]?.shut : undefined),
			answerLate: true,
		},
	);
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	toBottom();
	await expect.poll(() => lists().length).toBe(2);
	window.scrollTo({ top: 0, behavior: "instant" });
	// The next batch answers while the dismissal that overtook it is out.
	await dismissBench(0).click();
	await expect.poll(landed).toBe(1);
	reads[0]?.open();
	await wait(100);
	answers.shift()?.();
	await expect.poll(() => lists().length).toBe(3);
	expect(lists()[2]?.searchParams.get("offset")).toBe(String(perPage - 1));
	// The batch that replaced it is overtaken in turn, by the dismissal undone.
	await alertsTable()
		.getByRole("button", { name: "Reactivate the alert on bench-0 Latency" })
		.click();
	await expect.poll(landed).toBe(2);
	reads[1]?.open();
	await wait(100);
	answers.shift()?.();
	await expect.poll(() => lists().length).toBe(4);
	expect(lists()[3]?.searchParams.get("offset")).toBe(String(perPage));
	await wait(100);
	expect(await namesTopToBottom()).toEqual(
		Array.from({ length: perPage }, (_, id) => `${id + 2} bench-${id} Latency`),
	);
	reads[2]?.open();

	await everyAlertOnce(total);
});

// Kills a new view's first batch held back as overtaken when a change is out
// while it loads, which leaves the view with no rows at all.
test("a new view's first batch draws while a dismissal is still out", async () => {
	const perPage = alertsBatch();
	let answer = () => {};
	const { api: answered, cache } = dismissible(
		2 * perPage + 4,
		() =>
			new Promise<void>((resolve) => {
				answer = resolve;
			}),
	);
	const history = mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	await dismissBench(0).click();
	history.set({ value: `${PATH}?status=all` });
	await expect.element(dismissBench(1)).toBeVisible();
	answer();
});

// Kills later batches that follow a new view with their old offsets before
// they are let go, which asks the API for batches the new view never drew.
test("a new view after three batches asks only for its first batch", async () => {
	const perPage = alertsBatch();
	const { api: answered, cache, lists } = dismissible(2 * perPage + 4);
	const history = mount(answered, cache);
	await expect
		.poll(
			() => {
				toBottom();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	window.scrollTo({ top: 0, behavior: "instant" });
	await wait(100);
	history.set({ value: `${PATH}?status=active` });
	await expect.element(dismissBench(0)).toBeVisible();
	await wait(200);
	expect(
		lists()
			.slice(3)
			.map((url) => url.searchParams.get("status")),
	).toEqual(["active"]);
});

const offsetsOf = (urls: URL[]) =>
	urls.map((url) => url.searchParams.get("offset"));
const numbered = (from: number, to: number) =>
	Array.from({ length: to - from + 1 }, (_, index) => from + index);

// Kills a later batch read again on returning to the window, from the offset
// it was first asked from, which the dismissals ahead of it have moved: only
// the first batch is read again, and the next batch counts from it.
test("under Active, returning to the window after dismissals reads only the first batch again and draws every alert once", async () => {
	const perPage = alertsBatch();
	const total = 3 * perPage + 4;
	const { api: answered, cache, lists, landed } = dismissible(total);
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	await expect
		.poll(
			() => {
				toBottom();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	window.scrollTo({ top: 0, behavior: "instant" });
	for (const id of [0, 1, 2]) {
		await dismissBench(id).click();
	}
	await expect.poll(landed).toBe(3);
	const before = lists().length;
	refocus();
	try {
		await expect.poll(() => lists().length).toBeGreaterThan(before);
		await wait(200);
		expect(offsetsOf(lists().slice(before))).toEqual(["0"]);
	} finally {
		focusManager.setFocused(undefined);
	}

	await everyAlertOnce(total);
	expect(offsetsOf(lists().slice(before + 1))).toEqual([
		String(3 * perPage - 3),
	]);
});

// Kills a reread after a change the API made to no alert that reads the later
// batches again from the offsets they were first asked from, or before the
// first batch answers: each is asked for again from the offset the batches
// before it now count.
test("under Active, a reread after a change the API made to no alert asks each later batch again from the offset the first batch's answer counts", async () => {
	const perPage = alertsBatch();
	const total = 3 * perPage + 4;
	const first = gate();
	let holding = false;
	const {
		api: answered,
		cache,
		lists,
		landed,
		elsewhere,
	} = dismissible(total, undefined, {
		holdList: (offset) => (holding && offset === 0 ? first.shut : undefined),
	});
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	for (const id of [0, 1, 2]) {
		await dismissBench(id).click();
	}
	await expect.poll(landed).toBe(3);
	await expect
		.poll(
			() => {
				toBottom();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	expect(offsetsOf(lists())).toEqual([
		"0",
		String(perPage - 3),
		String(2 * perPage - 3),
	]);
	window.scrollTo({ top: 0, behavior: "instant" });

	// Another reader dismissed bench-4 first, so this dismissal changes nothing.
	elsewhere(4);
	holding = true;
	const before = lists().length;
	await dismissBench(4).click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Some alerts did not change");
	await expect.poll(() => lists().length).toBeGreaterThan(before);
	await wait(200);
	expect(offsetsOf(lists().slice(before))).toEqual(["0"]);
	holding = false;
	first.open();
	await expect.poll(() => lists().length).toBe(before + 2);
	expect(offsetsOf(lists().slice(before + 1))).toEqual([String(perPage)]);

	await drawnOnce([0, 1, 2, 3, ...numbered(5, total - 1)]);
});

// Kills a first batch read again on returning to the window whose fresh
// answer pushes its last rows past its end while the later batches keep their
// old answers, so no batch draws the rows pushed out: the list reads itself
// again from the top, the next batch from the offset the fresh first batch
// counts.
test("under Active, alerts raised ahead while the reader was away push no alert out of the list on returning to the window", async () => {
	const perPage = alertsBatch();
	const total = 3 * perPage + 4;
	const { api: answered, cache, lists, arrive } = dismissible(total);
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	await expect
		.poll(
			() => {
				toBottom();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	window.scrollTo({ top: 0, behavior: "instant" });

	arrive(total, total + 1);
	// The reader was away long enough for every batch to go stale.
	await cache.invalidateQueries({
		queryKey: ["console", "threshold", "hashbrown", THRESHOLD, "alerts"],
		refetchType: "none",
	});
	const before = lists().length;
	refocus();
	try {
		await expect.poll(() => lists().length).toBeGreaterThanOrEqual(before + 2);
		await wait(200);
		expect(offsetsOf(lists().slice(before))).toEqual(["0", String(perPage)]);
	} finally {
		focusManager.setFocused(undefined);
	}

	await drawnOnce([total, total + 1, ...numbered(0, total - 1)]);
});

// Kills a first batch's fresh answer checked against every row its old answer
// held, the row dismissed here included, which left the view: one alert
// raised ahead of it then has the whole list read again.
test("under Active, an alert raised ahead of a row dismissed here reads only the first batch again on returning to the window", async () => {
	const perPage = alertsBatch();
	const total = 3 * perPage + 4;
	const { api: answered, cache, lists, landed, arrive } = dismissible(total);
	mount(answered, cache, `${PATH}?status=active`);
	await expect.element(dismissBench(0)).toBeVisible();
	await expect
		.poll(
			() => {
				toBottom();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	window.scrollTo({ top: 0, behavior: "instant" });
	await dismissBench(2).click();
	await expect.poll(landed).toBe(1);

	arrive(total);
	const before = lists().length;
	refocus();
	try {
		await expect.poll(() => lists().length).toBeGreaterThan(before);
		await wait(200);
		expect(offsetsOf(lists().slice(before))).toEqual(["0"]);
	} finally {
		focusManager.setFocused(undefined);
	}

	await drawnOnce([total, ...numbered(0, total - 1)]);
});

// Kills a card drawn on the raised surface, as the default card is.
test("every card on the page is soft", async () => {
	const { api: answered } = api();
	mount(answered, client());

	await expect
		.element(page.getByRole("region", { name: "Model history" }))
		.toBeVisible();
	const cards = [...document.querySelectorAll(".th-card")];
	expect(cards.length).toBeGreaterThan(1);
	expect(cards.map((card) => card.getAttribute("data-variant"))).toEqual(
		cards.map(() => "soft"),
	);
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
	const report = rows.nth(1).getByRole("link", { name: /, open the report$/ });
	await expect
		.element(report)
		.toHaveAttribute(
			"href",
			"/next/console/projects/hashbrown/reports/report-new",
		);
	expectNameStartsWithText(report.element());
	await expect
		.element(rows.nth(1).getByRole("img", { name: "alerting" }))
		.toBeVisible();
	await expect.element(rows.nth(2)).toHaveTextContent(/Dismissed Aug 27/);
	expect(rows.nth(2).getByRole("img", { name: "alerting" }).query()).toBeNull();
	await expect
		.element(rows.nth(3))
		.toHaveTextContent(/Silenced: branch head replaced/);
	await expect
		.element(page.getByText("1 active · 2 dismissed", { exact: true }))
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
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
			return { data: thresholdFixture() };
		}
		if (url.pathname.endsWith("/v0/console/projects/hashbrown/alerts")) {
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
		.element(page.getByRole("heading", { name: "No alerts" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "Open in Explore" }))
		.toBeDisabled();
});

// Kills a summary that keeps its first answer, and a Window control whose
// pick never reaches the API or the link.
test("the summary follows the window's counts", async () => {
	const { api: answered, requests } = fakeApi((url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
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
	await expect.element(summary()).toHaveTextContent(/^1 active · 2 dismissed$/);

	await page.getByRole("radio", { name: "1w" }).click();
	await expect.element(summary()).toHaveTextContent(/^1 active · 0 dismissed$/);
	expect(history.get()).toBe(`${PATH}?window=1w`);
	const week = requests.filter(isAlerts).at(-1);
	expect(week?.searchParams.get("window")).toBe("7");
	expect(
		Math.round(
			(Date.now() - Number(week?.searchParams.get("start_time"))) / DAY,
		),
	).toBe(7);

	await page.getByRole("radio", { name: "Custom" }).click();
	await expect.element(summary()).toHaveTextContent(/^0 active · 0 dismissed$/);
	expect(requests.filter(isAlerts).at(-1)?.searchParams.has("end_time")).toBe(
		true,
	);
});

// Kills a title that keeps its first answer when the threshold answers again.
test("the title follows its threshold when it answers again", async () => {
	let branch = "main";
	const { api: answered } = fakeApi((url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
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

// Kills an alert row whose Explore link opens another line or window than the
// page's Open in Explore does when the row's line is the only one listed.
test("an alert row's Open in Explore is the page's link for its line alone", async () => {
	const lone = alertsFixture();
	const { api: answered } = fakeApi((url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
			return { data: thresholdFixture() };
		}
		return isAlerts(url)
			? {
					data: {
						...lone,
						total: 1,
						counts: { active: 1, dismissed: 0, silenced: 0 },
						groups: lone.groups.slice(0, 1),
					},
				}
			: new Promise<never>(() => {});
	});
	mount(answered, client(), `${PATH}?window=1w`);
	const header = page.getByRole("link", { name: EXPLORE });
	await expect.element(header).toBeVisible();
	await userEvent.hover(
		alertsTable().getByRole("columnheader", { name: "Line" }),
	);
	await userEvent.hover(
		alertsTable().getByRole("columnheader", { name: "Limit" }),
	);
	await expect
		.element(alertsTable().getByRole("link", { name: /^Open in Explore: / }))
		.toHaveAttribute("href", header.element().getAttribute("href") as string);
});

// Kills an archived threshold called active, and an Applies to card that does
// not say which dimension archived it.
test("an archived threshold says so, and what archived it", async () => {
	const { api: answered } = fakeApi((url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
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

// Kills a host read from anywhere but the project the page is in.
test("the run snippet names the project's self-hosted API", async () => {
	const { api: answered } = api();
	mount(answered, client(), PATH, "http://localhost:61016");
	const change = page.getByRole("region", { name: "Change this threshold" });
	await expect
		.element(change)
		.toHaveTextContent("--host http://localhost:61016");
});

// Kills alerts that never load past their first batch, or ask again for a
// batch they have.
test("scrolling to the end loads each next batch of alerts once", async () => {
	const perPage = alertsBatch();
	const total = 3 * perPage;
	const { api: answered, requests } = fakeApi((url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
			return { data: thresholdFixture() };
		}
		if (isAlerts(url)) {
			const offset = Number(url.searchParams.get("offset"));
			return { data: alertsPage(offset, perPage, total) };
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
	expect([0, 1, 2, 3].map((batch) => asked(requests, batch * perPage))).toEqual(
		[1, 1, 1, 0],
	);
});

// Kills a later batch of alerts the API refused that fails without a word, or
// whose Retry asks for nothing.
test("a later batch of alerts the API refused offers Retry, which loads it", async () => {
	const perPage = alertsBatch();
	let refuse = true;
	const { api: answered, requests } = fakeApi((url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
			return { data: thresholdFixture() };
		}
		if (isAlerts(url)) {
			const offset = Number(url.searchParams.get("offset"));
			if (offset === perPage && refuse) {
				return Promise.reject(new ApiError(429, "client", "Too many requests"));
			}
			return { data: alertsPage(offset, perPage, 2 * perPage) };
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
	expect(asked(requests, perPage)).toBe(1);

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
	expect(asked(requests, perPage)).toBe(2);
});

// Kills the last threshold's alerts drawn as the next one's while its own load.
test("moving to another threshold never shows the last one's alerts as its own", async () => {
	const OTHER = "00000000-0000-4000-8000-0000000000b2";
	let release: (() => void) | undefined;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const { api: answered } = fakeApi(async (url) => {
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${THRESHOLD}`,
			)
		) {
			return { data: thresholdFixture() };
		}
		if (
			url.pathname.endsWith(
				`/v0/console/projects/hashbrown/thresholds/${OTHER}`,
			)
		) {
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
		.element(page.getByRole("heading", { name: "No alerts" }))
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
