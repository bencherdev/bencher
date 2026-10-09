import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "../report/report.css";
import "../reports/reports.css";
import "../alerts/alerts.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonConsoleAlerts,
	JsonConsoleProject,
} from "../../types/bencher";
import { type Api, ApiError, type Method } from "../api";
import { READ, alertsFixture } from "../alerts/testing";
import { budget } from "../budget";
import { createQueryClient } from "../cache";
import { NEXT_PROJECTS } from "../paths";
import { ProjectContext } from "../project";
import { consoleProjectQuery } from "../queries";
import { bootstrapOf } from "../settings/testing";
import Alerts from "./Alerts";

const PATH = `${NEXT_PROJECTS}/hashbrown/alerts`;
const NAME = "blake3 n=0 Latency";
const ALL = "the alerts in the report on main, ubuntu-latest, Sep 13, 21:16";

let dispose: (() => void) | undefined;

interface Sent {
	method: Method;
	path: string;
	body: unknown;
}

/** An API that answers the list with `list` and each change with `change`, recording both. */
const fakeApi = (
	list: (url: URL) => JsonConsoleAlerts | Promise<JsonConsoleAlerts>,
	change: (sent: Sent) => Promise<{ changed: number }> = async (sent) => ({
		changed: (sent.body as { alerts?: string[] }).alerts?.length ?? 1,
	}),
) => {
	const reads: URL[] = [];
	const sent: Sent[] = [];
	let answered = 0;
	const api: Api = {
		get: async <T,>(path: string) => {
			const url = new URL(path, "http://api.test");
			reads.push(url);
			if (!url.pathname.endsWith("/console/alerts")) {
				return new Promise<never>(() => {});
			}
			return { data: (await list(url)) as T, headers: new Headers() };
		},
		send: async <T,>(method: Method, path: string, body?: unknown) => {
			const each = { method, path, body };
			sent.push(each);
			try {
				return { data: (await change(each)) as T, headers: new Headers() };
			} finally {
				answered += 1;
			}
		},
	};
	const lists = () =>
		reads.filter((url) => url.pathname.endsWith("/console/alerts"));
	return { api, reads, lists, sent, answered: () => answered };
};

const newClient = (api: Api, bootstrap: JsonConsoleProject) => {
	const client = createQueryClient(() => {});
	client.setDefaultOptions({
		queries: { ...client.getDefaultOptions().queries, retry: false },
	});
	client.setQueryData(
		consoleProjectQuery(api, "hashbrown").queryKey,
		bootstrap,
	);
	return client;
};

const mount = (
	api: Api,
	bootstrap: JsonConsoleProject = { ...bootstrapOf(), active_alerts: 3 },
	path = PATH,
	client = newClient(api, bootstrap),
) => {
	const history = createMemoryHistory();
	history.set({ value: path });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<MemoryRouter
					history={history}
					base={NEXT_PROJECTS}
					root={(props) => (
						<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
							<Suspense fallback={<p>Suspended</p>}>{props.children}</Suspense>
						</ProjectContext.Provider>
					)}
				>
					<Route path="/:project/alerts/*rest" component={Alerts} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return client;
};

const badge = (client: QueryClient) =>
	client.getQueryData<JsonConsoleProject>(["console", "project", "hashbrown"])
		?.active_alerts;
const heading = () => page.getByRole("heading", { level: 1 });
const dismissButtons = () =>
	page.getByRole("button", { name: `Dismiss the alert on ${NAME}` });
/** Each drawn alert row's status, top to bottom: the alert number and what its controls say. */
const rowStates = () =>
	[...document.querySelectorAll("tbody tr[aria-rowindex]")]
		.filter((row) => row.querySelector("td.lr-act"))
		.map((row) => row.querySelector("td.lr-act")?.textContent ?? "");

const three = () =>
	alertsFixture([
		{
			report: 0,
			alerts: [{ alert: 1 }, { alert: 2 }, { alert: 3 }],
		},
	]);

beforeEach(async () => {
	await page.viewport(1280, 720);
	window.scrollTo(0, 0);
});

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills a dismiss that waits on the API, a refusal that leaves the row, the
// count, or the badge moved, and one that says nothing.
test("a refused dismiss puts back the row, the count, and the badge, and says so", async () => {
	let refuse: (() => void) | undefined;
	const { api } = fakeApi(
		three,
		() =>
			new Promise((_, reject) => {
				refuse = () => reject(new ApiError(500, "server", "{}"));
			}),
	);
	const client = mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(1).click();

	await expect.element(page.getByText("Dismissed just now")).toBeVisible();
	await expect.element(heading()).toHaveTextContent("2 active");
	expect(badge(client)).toBe(2);

	refuse?.();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"The alerts did not change: the Bencher API did not answer.",
		);
	await expect.element(heading()).toHaveTextContent("3 active");
	expect(badge(client)).toBe(3);
	expect(rowStates()).toEqual(["Dismiss", "Dismiss", "Dismiss"]);
});

// Kills a refused dismiss, made after its undo, that moves the counts or the
// badge a second time.
test("a refused dismiss after its undo moves nothing twice", async () => {
	const answers: (() => void)[] = [];
	const { api } = fakeApi(
		three,
		(sent) =>
			new Promise((resolve, reject) => {
				answers.push(() =>
					(sent.body as { status: string }).status === "dismissed"
						? reject(new ApiError(500, "server", "{}"))
						: resolve({ changed: 0 }),
				);
			}),
	);
	const client = mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().first().click();
	await page
		.getByRole("button", { name: `Reactivate the alert on ${NAME}` })
		.click();
	await expect.element(heading()).toHaveTextContent("3 active");
	expect(badge(client)).toBe(3);

	answers[0]?.();
	await expect.element(page.getByRole("alert")).toBeVisible();
	await expect.element(heading()).toHaveTextContent("3 active");
	expect(badge(client)).toBe(3);
	answers[1]?.();
	await expect
		.poll(() => rowStates())
		.toEqual(["Dismiss", "Dismiss", "Dismiss"]);
	await expect.element(heading()).toHaveTextContent("3 active");
	expect(badge(client)).toBe(3);
});

// Kills a dismissed row that vanishes, or jumps to the end, once the list is
// read again without it.
test("a row dismissed in place keeps its place when the list is read again without it", async () => {
	let reads = 0;
	const { api } = fakeApi(() => {
		reads += 1;
		return reads === 1
			? three()
			: alertsFixture([{ report: 0, alerts: [{ alert: 1 }, { alert: 3 }] }]);
	});
	const client = mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(1).click();
	await expect.element(heading()).toHaveTextContent("2 active");
	await client.refetchQueries({ queryKey: ["console", "alerts"] });
	expect(reads).toBe(2);
	await expect
		.poll(() => rowStates())
		.toEqual(["Dismiss", "Dismissed just now·Reactivate", "Dismiss"]);
	await expect.element(heading()).toHaveTextContent("2 active");
});

// Kills a Dismiss all that counts the rows loaded rather than every matching
// alert, skips its confirmation, selects other alerts than the list counted, or
// reaches alerts raised after the API read the list.
test("Dismiss all confirms the count of every matching alert and sends the list's own filter", async () => {
	let answer: (() => void) | undefined;
	const { api, reads, sent } = fakeApi(
		() =>
			({
				...three(),
				total: 40,
				counts: { active: 40, dismissed: 0, silenced: 0 },
			}) as JsonConsoleAlerts,
		() =>
			new Promise((resolve) => {
				answer = () => resolve({ changed: 40 });
			}),
	);
	const client = mount(api, { ...bootstrapOf(), active_alerts: 40 });
	await page
		.getByRole("button", {
			name: "Dismiss all 40 active alerts that match the filters",
		})
		.click();
	const dialog = page.getByRole("dialog", {
		name: "Dismiss 40 active alerts?",
	});
	await expect.element(dialog).toBeVisible();
	expect(sent).toHaveLength(0);
	await dialog.getByRole("button", { name: "Dismiss 40 alerts" }).click();

	// The alerts not loaded leave the count before the API answers, as the loaded ones do.
	await expect.element(heading()).toHaveTextContent("0 active");
	expect(badge(client)).toBe(0);
	answer?.();
	await expect.poll(() => sent.length).toBe(1);
	await expect.element(heading()).toHaveTextContent("0 active");
	const listed = reads.find((url) => url.pathname.endsWith("/console/alerts"));
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/projects/hashbrown/console/alerts",
			body: {
				status: "dismissed",
				filter: {
					status: "active",
					start_time: Number(listed?.searchParams.get("start_time")),
					end_time: READ,
				},
			},
		},
	]);
	expect(badge(client)).toBe(0);
	expect(rowStates()).toEqual([
		"Dismissed just now·Reactivate",
		"Dismissed just now·Reactivate",
		"Dismissed just now·Reactivate",
	]);
});

// Kills a group checkbox that cannot show a partial selection, or that
// clears a partial group instead of filling it.
test("a group's checkbox shows a partial selection and fills the group", async () => {
	const { api } = fakeApi(three);
	mount(api);
	const group = page.getByRole("checkbox", { name: `Select ${ALL}` });
	await page
		.getByRole("checkbox", { name: `Select ${NAME}` })
		.nth(0)
		.click();
	await expect.element(group).not.toBeChecked();
	expect((group.element() as HTMLInputElement).indeterminate).toBe(true);
	await expect.element(page.getByText("1 selected")).toBeVisible();

	await group.click();
	await expect.element(group).toBeChecked();
	expect((group.element() as HTMLInputElement).indeterminate).toBe(false);
	await expect.element(page.getByText("3 selected")).toBeVisible();
});

// Kills status controls drawn for a reader who cannot use them, and
// selection taken away with them.
test("a viewer gets no status controls, and can still select", async () => {
	const { api } = fakeApi(() =>
		alertsFixture([
			{
				report: 0,
				alerts: [{ alert: 1 }, { alert: 2, status: "dismissed" }],
			},
		]),
	);
	mount(
		api,
		{ ...bootstrapOf({ edit: false }), active_alerts: 1 },
		`${PATH}?status=all`,
	);
	await expect
		.element(page.getByText("Read only. Ask a project Maintainer for access."))
		.toBeVisible();
	await expect.element(page.getByText(/^Dismissed /)).toBeVisible();
	for (const name of [/^Dismiss/, /^Reactivate/]) {
		await expect
			.element(page.getByRole("button", { name }))
			.not.toBeInTheDocument();
	}
	await page.getByRole("checkbox", { name: `Select ${ALL}` }).click();
	await expect.element(page.getByText("2 selected")).toBeVisible();
});

// Kills a next batch asked for by page number, or one that starts past the
// alerts the page dismissed made room for.
test("the next batch starts after the rows still in the view", async () => {
	await page.viewport(1280, 400);
	const many = (from: number, count: number) =>
		({
			...alertsFixture([
				{
					report: 0,
					alerts: Array.from({ length: count }, (_, index) => ({
						alert: from + index,
					})),
				},
			]),
			total: 30,
			counts: { active: 30, dismissed: 0, silenced: 0 },
		}) as JsonConsoleAlerts;
	const { api, reads } = fakeApi((url) => {
		const offset = Number(url.searchParams.get("offset"));
		const size = Number(url.searchParams.get("per_page"));
		return many(offset + 1, Math.min(size, 30 - offset));
	});
	mount(api, { ...bootstrapOf(), active_alerts: 30 });
	await expect.element(heading()).toHaveTextContent("30 active");
	await dismissButtons().nth(0).click();
	await dismissButtons().nth(0).click();
	await expect.element(heading()).toHaveTextContent("28 active");
	const lists = () =>
		reads.filter((url) => url.pathname.endsWith("/console/alerts"));
	expect(lists()).toHaveLength(1);
	const size = Number(lists()[0]?.searchParams.get("per_page"));

	window.scrollTo({
		top: document.documentElement.scrollHeight,
		behavior: "instant",
	});
	await expect.poll(() => lists().length).toBe(2);
	expect(lists()[1]?.searchParams.get("offset")).toBe(String(size - 2));
	expect(lists()[1]?.searchParams.has("page")).toBe(false);
});

// One frame at 120 Hz.
const FRAME_MS = 8.3;
const RUNS = 9;

// Kills a dismiss or a reactivate whose own redraw takes more than one 120 Hz frame of main thread work.
test("a dismiss and its undo each redraw within a frame", async () => {
	const { api, answered } = fakeApi(() =>
		alertsFixture([
			{
				report: 0,
				alerts: Array.from({ length: 30 }, (_, index) => ({
					alert: index + 1,
				})),
			},
		]),
	);
	mount(api, { ...bootstrapOf(), active_alerts: 30 });
	await expect.element(heading()).toHaveTextContent("30 active");
	const time = async (name: RegExp) => {
		const button = page
			.getByRole("button", { name })
			.first()
			.element() as HTMLElement;
		const before = answered();
		const start = performance.now();
		button.click();
		document.body.getBoundingClientRect();
		const spent = performance.now() - start;
		await expect.poll(answered).toBe(before + 1);
		await new Promise((resolve) => requestAnimationFrame(resolve));
		return spent;
	};
	const dismisses: number[] = [];
	const undos: number[] = [];
	for (let run = 0; run < RUNS; run++) {
		dismisses.push(await time(/^Dismiss the alert on /));
		await expect.element(heading()).toHaveTextContent("29 active");
		undos.push(await time(/^Reactivate the alert on /));
		await expect.element(heading()).toHaveTextContent("30 active");
	}
	const median = (values: number[]) =>
		[...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ??
		Number.NaN;
	console.log(
		`speed alerts dismiss: median ${median(dismisses).toFixed(2)} ms, undo ${median(undos).toFixed(2)} ms, budget ${budget(FRAME_MS).toFixed(2)} ms`,
	);
	expect(median(dismisses)).toBeLessThan(budget(FRAME_MS));
	expect(median(undos)).toBeLessThan(budget(FRAME_MS));
});

const WHERE = "main, ubuntu-latest, Sep 13, 21:16";
const SHA256 = "sha256 n=0 Latency";
const status = (name: string) =>
	page.getByRole("radiogroup", { name: "Status" }).getByRole("radio", { name });
const focusedName = () =>
	document.activeElement?.getAttribute("aria-label") ??
	document.activeElement?.textContent ??
	"";
/** Press Enter on a control, as a keyboard reader does. */
const press = async (control: ReturnType<typeof page.getByRole>) => {
	(control.element() as HTMLElement).focus();
	await userEvent.keyboard("{Enter}");
};
const alertQueries = (client: QueryClient) =>
	client
		.getQueryCache()
		.findAll({ queryKey: ["console", "alerts", "hashbrown"] });
/** The active count in the cached first batch of the view `search` names. */
const firstCount = (client: QueryClient, search: string) =>
	alertQueries(client).find(
		({ queryKey }) => queryKey[3] === search && queryKey[5] === 0,
	)?.state.data as { alerts: JsonConsoleAlerts } | undefined;
const leave = () => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
};

// Kills a confirmed change left out of the view's cached batches and counts,
// which a return then shows as active, a view never read again on that
// return, and one read again while the reader stays.
test("a dismissed alert reads as dismissed when the reader comes back, before the list is read again", async () => {
	const { api, lists, answered } = fakeApi((url) =>
		url.searchParams.get("offset") === "0" && lists().length > 1
			? new Promise<never>(() => {})
			: three(),
	);
	const client = mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(1).click();
	await expect.poll(answered).toBe(1);
	await expect
		.poll(() => alertQueries(client).every(({ state }) => state.isInvalidated))
		.toBe(true);
	expect(lists()).toHaveLength(1);

	leave();
	mount(api, undefined, PATH, client);
	await expect.element(heading()).toHaveTextContent("2 active");
	expect(rowStates()).toEqual(["Dismiss", "Dismiss"]);
	await expect.poll(() => lists().length).toBe(2);
});

// Kills counts patched in whatever view is on screen when the API answers.
test("a change answered after the reader switched views moves the counts of the view it was made in", async () => {
	let answer: (() => void) | undefined;
	const { api } = fakeApi(
		three,
		() =>
			new Promise((resolve) => {
				answer = () => resolve({ changed: 1 });
			}),
	);
	const client = mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(1).click();
	await status("All").click();
	await expect.poll(() => firstCount(client, "status=all")).toBeTruthy();
	answer?.();
	await expect.poll(() => firstCount(client, "")?.alerts.counts.active).toBe(2);
	expect(firstCount(client, "status=all")?.alerts.counts.active).toBe(3);
});

// Kills a count or a tab badge below zero when the API changed more than the
// page counted, and a badge left at the page's guess rather than corrected by
// what the API changed.
test.each([
	[41, "0 active", 0],
	[30, "10 active", 10],
])(
	"Dismiss all answered with %i changed reads %s",
	async (changed, text, active) => {
		const { api, answered } = fakeApi(
			() =>
				({
					...three(),
					total: 40,
					counts: { active: 40, dismissed: 0, silenced: 0 },
				}) as JsonConsoleAlerts,
			async () => ({ changed }),
		);
		const client = mount(api, { ...bootstrapOf(), active_alerts: 40 });
		await page
			.getByRole("button", {
				name: "Dismiss all 40 active alerts that match the filters",
			})
			.click();
		await page
			.getByRole("dialog", { name: "Dismiss 40 active alerts?" })
			.getByRole("button", { name: "Dismiss 40 alerts" })
			.click();
		await expect.poll(answered).toBe(1);
		await expect.element(heading()).toHaveTextContent(text);
		expect(badge(client)).toBe(active);
	},
);

// Kills a Dismiss all or a Dismiss group that selects by the new search's
// filters while the rows on screen are still the last search's.
test("while a new search loads, Dismiss all and Dismiss group wait", async () => {
	const { api } = fakeApi((url) =>
		url.searchParams.get("status") === "all"
			? new Promise<never>(() => {})
			: three(),
	);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const group = page.getByRole("button", { name: `Dismiss group on ${WHERE}` });
	const all = page.getByRole("button", { name: /^Dismiss all / });
	await expect.element(group).toBeEnabled();
	await expect.element(all).toBeEnabled();
	await status("All").click();
	await expect.element(group).toBeDisabled();
	await expect.element(all).toBeDisabled();
});

// Kills a change the API made for fewer alerts than the page loaded that
// still shows every row changed, says nothing, or never reads the list again.
test("a change the API made for fewer alerts than were loaded puts the rows back, says so, and reads the list again", async () => {
	const { api, lists } = fakeApi(three, async () => ({ changed: 1 }));
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await page.getByRole("button", { name: `Dismiss group on ${WHERE}` }).click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"Some alerts did not change, so the list is read again.",
		);
	await expect
		.poll(() => rowStates())
		.toEqual(["Dismiss", "Dismiss", "Dismiss"]);
	await expect.poll(() => lists().length).toBe(2);
});

// Kills rows drawn anew when the API's answer is written, which drops the
// focus a row's Dismiss handed to its Reactivate.
test("a row's Reactivate keeps the focus once the API answers its Dismiss", async () => {
	const { api, answered } = fakeApi(three);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const row = dismissButtons().nth(1).element().closest("tr");
	await press(dismissButtons().nth(1));
	await expect.poll(answered).toBe(1);
	await new Promise((resolve) => requestAnimationFrame(resolve));
	expect(focusedName()).toBe(`Reactivate the alert on ${NAME}`);
	expect(document.activeElement?.closest("tr")).toBe(row);
});

// Kills a refusal that drops the focus with the Reactivate it takes away.
test("a refused dismiss puts the focus back on the row's Dismiss", async () => {
	let refuse: (() => void) | undefined;
	const { api } = fakeApi(
		three,
		() =>
			new Promise((_, reject) => {
				refuse = () => reject(new ApiError(500, "server", "{}"));
			}),
	);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const row = dismissButtons().nth(1).element().closest("tr");
	await press(dismissButtons().nth(1));
	await expect.poll(focusedName).toBe(`Reactivate the alert on ${NAME}`);
	refuse?.();
	await expect.poll(focusedName).toBe(`Dismiss the alert on ${NAME}`);
	expect(document.activeElement?.closest("tr")).toBe(row);
});

// Kills Dismiss N dropping the focus with the button it takes away.
test("after Dismiss N the focus rests on the selection count", async () => {
	const { api, answered } = fakeApi(three);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await page
		.getByRole("checkbox", { name: `Select ${NAME}` })
		.nth(0)
		.click();
	await press(
		page.getByRole("button", { name: "Dismiss 1 selected active alert" }),
	);
	await expect.poll(answered).toBe(1);
	await expect.poll(focusedName).toBe("1 selected");
});

// Kills Dismiss group dropping the focus once its group has no active row,
// and a header drawn anew on the change.
test("after Dismiss group the focus rests on the group's checkbox", async () => {
	const { api, answered } = fakeApi(three, async () => ({ changed: 3 }));
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const group = page.getByRole("button", { name: `Dismiss group on ${WHERE}` });
	const header = group.element().closest("tr");
	await press(group);
	await expect.poll(answered).toBe(1);
	await new Promise((resolve) => requestAnimationFrame(resolve));
	expect(focusedName()).toBe(`Select ${ALL}`);
	expect(document.activeElement?.closest("tr")).toBe(header);
});

// Kills Dismiss all dropping the focus with the button it disables.
test("after Dismiss all the focus rests on the count", async () => {
	const { api, answered } = fakeApi(three, async () => ({ changed: 3 }));
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await page
		.getByRole("button", {
			name: "Dismiss all 3 active alerts that match the filters",
		})
		.click();
	await press(
		page
			.getByRole("dialog", { name: "Dismiss 3 active alerts?" })
			.getByRole("button", { name: "Dismiss 3 alerts" }),
	);
	await expect.poll(answered).toBe(1);
	await expect.poll(() => document.activeElement).toBe(heading().element());
});

const dismissedPair = () =>
	alertsFixture([
		{
			report: 0,
			alerts: [
				{ alert: 1, status: "dismissed" },
				{ alert: 2, status: "dismissed", benchmark: 1 },
			],
		},
	]);

// Kills a Reactivate under Dismissed that drops the focus with the row it
// takes away.
test("after Reactivate under Dismissed the focus rests on the row now in its place", async () => {
	const { api, answered } = fakeApi(dismissedPair);
	mount(
		api,
		{ ...bootstrapOf(), active_alerts: 0 },
		`${PATH}?status=dismissed`,
	);
	const reactivate = page.getByRole("button", {
		name: `Reactivate the alert on ${NAME}`,
	});
	await expect.element(reactivate).toBeVisible();
	await press(reactivate);
	await expect.poll(answered).toBe(1);
	await expect.poll(focusedName).toBe(`Select ${SHA256}`);
});

// Kills rows that fold only at phone width, which leaves a row's controls off
// screen on a tablet, and a folded Reactivate that drops the focus with the
// line it takes away.
test("rows fold below 980 px, and a folded Reactivate leaves the focus on its row", async () => {
	await page.viewport(900, 800);
	const { api, answered } = fakeApi(() =>
		alertsFixture([
			{
				report: 0,
				alerts: [{ alert: 1 }, { alert: 2, status: "dismissed", benchmark: 1 }],
			},
		]),
	);
	mount(api, { ...bootstrapOf(), active_alerts: 1 }, `${PATH}?status=all`);
	const reactivate = page.getByRole("button", {
		name: `Reactivate the alert on ${SHA256}`,
	});
	await expect.element(reactivate).toBeVisible();
	expect(document.querySelector("table.lr-narrow")).not.toBeNull();
	await press(reactivate);
	await expect.poll(answered).toBe(1);
	await expect.poll(focusedName).toBe(`Select ${SHA256}`);
	await page.viewport(1280, 720);
	await expect.poll(() => document.querySelector("table.lr-narrow")).toBeNull();
});

// Kills a report or threshold page, or the Reports list, left showing a
// status the API no longer holds, and another project's pages read again.
test("a confirmed change has the report, Reports, and threshold pages read again", async () => {
	const { api, answered } = fakeApi(three);
	const client = mount(api);
	const kinds = ["report", "reports", "threshold", "thresholds"];
	for (const kind of kinds) {
		client.setQueryData(["console", kind, "hashbrown", "x"], {});
	}
	client.setQueryData(["console", "report", "other", "x"], {});
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(1).click();
	await expect.poll(answered).toBe(1);
	await expect
		.poll(() =>
			kinds.map(
				(kind) =>
					client.getQueryState(["console", kind, "hashbrown", "x"])
						?.isInvalidated,
			),
		)
		.toEqual([true, true, true, true]);
	expect(
		client.getQueryState(["console", "report", "other", "x"])?.isInvalidated,
	).toBe(false);
});

// Kills a Dismiss group whose count waits for the API to move the group's
// alerts that are not loaded.
test("Dismiss group takes the group's alerts not loaded out of the count at once", async () => {
	const { api } = fakeApi(
		() =>
			({
				...alertsFixture([
					{
						report: 0,
						total: 10,
						alerts: [{ alert: 1 }, { alert: 2 }, { alert: 3 }],
					},
				]),
				total: 10,
				counts: { active: 10, dismissed: 0, silenced: 0 },
			}) as JsonConsoleAlerts,
		() => new Promise(() => {}),
	);
	mount(api, { ...bootstrapOf(), active_alerts: 10 });
	await expect.element(heading()).toHaveTextContent("10 active");
	await page.getByRole("button", { name: `Dismiss group on ${WHERE}` }).click();
	await expect.element(heading()).toHaveTextContent("0 active");
});

// Kills a next batch asked while a change is out, whose offset still counts
// the rows that change is taking out of the view.
test("the next batch waits for a change in flight", async () => {
	await page.viewport(1280, 400);
	const many = (from: number, count: number) =>
		({
			...alertsFixture([
				{
					report: 0,
					alerts: Array.from({ length: count }, (_, index) => ({
						alert: from + index,
					})),
				},
			]),
			total: 30,
			counts: { active: 30, dismissed: 0, silenced: 0 },
		}) as JsonConsoleAlerts;
	let answer: (() => void) | undefined;
	const { api, lists } = fakeApi(
		(url) => {
			const offset = Number(url.searchParams.get("offset"));
			const size = Number(url.searchParams.get("per_page"));
			return many(offset + 1, Math.min(size, 30 - offset));
		},
		() =>
			new Promise((resolve) => {
				answer = () => resolve({ changed: 1 });
			}),
	);
	mount(api, { ...bootstrapOf(), active_alerts: 30 });
	await expect.element(heading()).toHaveTextContent("30 active");
	await dismissButtons().nth(0).click();
	const size = Number(lists()[0]?.searchParams.get("per_page"));
	window.scrollTo({
		top: document.documentElement.scrollHeight,
		behavior: "instant",
	});
	for (let frames = 0; frames < 3; frames++) {
		await new Promise((resolve) => requestAnimationFrame(resolve));
	}
	expect(lists()).toHaveLength(1);
	answer?.();
	await expect.poll(() => lists().length).toBe(2);
	expect(lists()[1]?.searchParams.get("offset")).toBe(String(size - 1));
});

// Kills Reactivate N offered under Active, where a dismissed row has its own.
test("Active offers no Reactivate N", async () => {
	const { api, answered } = fakeApi(three);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(0).click();
	await expect.poll(answered).toBe(1);
	await page
		.getByRole("checkbox", { name: `Select ${NAME}` })
		.nth(0)
		.click();
	await expect.element(page.getByText("1 selected")).toBeVisible();
	await expect
		.element(page.getByRole("button", { name: /^Reactivate 1 selected/ }))
		.not.toBeInTheDocument();
});

// Kills Dismiss group offered on a report none of whose rows still alert.
test("a report with no active row offers no Dismiss group", async () => {
	const { api } = fakeApi(() =>
		alertsFixture([
			{ report: 0, alerts: [{ alert: 1, status: "dismissed" }] },
			{ report: 1, hours: 1, alerts: [{ alert: 2 }] },
		]),
	);
	mount(api, { ...bootstrapOf(), active_alerts: 1 }, `${PATH}?status=all`);
	await expect
		.element(
			page.getByRole("button", {
				name: "Dismiss group on main, ubuntu-latest, Sep 13, 20:16",
			}),
		)
		.toBeVisible();
	expect(
		page.getByRole("button", { name: `Dismiss group on ${WHERE}` }).elements(),
	).toHaveLength(0);
});

// Kills a dismissed row dimmed only once the API answers, or never.
test("a dismissed row turns the muted color at once", async () => {
	const { api } = fakeApi(three, () => new Promise(() => {}));
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(1).click();
	const probe = document.createElement("span");
	probe.style.color = "var(--color-text-muted)";
	document.querySelector(".console")?.append(probe);
	const muted = getComputedStyle(probe).color;
	const names = [...document.querySelectorAll("tbody tr.lr b")].map(
		(name) => getComputedStyle(name).color,
	);
	expect(names[1]).toBe(muted);
	expect(names[0]).not.toBe(muted);
});

// Kills a row dismissed under one status that no other status keeps in place.
test("a row dismissed under All stays in place under Active", async () => {
	const { api, answered } = fakeApi((url) =>
		url.searchParams.get("status") === "all"
			? three()
			: alertsFixture([{ report: 0, alerts: [{ alert: 1 }, { alert: 3 }] }]),
	);
	mount(api, undefined, `${PATH}?status=all`);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(1).click();
	await expect.poll(answered).toBe(1);
	await status("Active").click();
	await expect
		.poll(() => rowStates())
		.toEqual(["Dismiss", "Dismissed just now·Reactivate", "Dismiss"]);
});

// Kills a filtered page that counts the project's active alerts by its filters.
test("a filtered page names the project's active count from the tab badge", async () => {
	const { api } = fakeApi(three);
	mount(
		api,
		{ ...bootstrapOf(), active_alerts: 9 },
		`${PATH}?branch=6f3c1a2e-0000-4000-8000-000000000001`,
	);
	await expect.element(heading()).toHaveTextContent("3 active");
	await expect
		.element(page.getByText(/ · 9 active in the project$/))
		.toBeVisible();
});

// Kills a next batch from an offset already read, after alerts left the list
// elsewhere, keyed as that earlier batch: it serves the earlier answer again
// and the list stops short.
test("a batch from an offset already read is read again", async () => {
	await page.viewport(1280, 400);
	let gone = 0;
	const { api, lists } = fakeApi((url) => {
		const offset = Number(url.searchParams.get("offset"));
		const size = Number(url.searchParams.get("per_page"));
		const count = Math.max(0, Math.min(size, 90 - gone - offset));
		return {
			...alertsFixture([
				{
					report: 0,
					alerts: Array.from({ length: count }, (_, index) => ({
						alert: gone + offset + index + 1,
					})),
				},
			]),
			total: 90 - gone,
			counts: { active: 90 - gone, dismissed: gone, silenced: 0 },
		} as JsonConsoleAlerts;
	});
	const client = mount(api, { ...bootstrapOf(), active_alerts: 90 });
	await expect.element(heading()).toHaveTextContent("90 active");
	const size = Number(lists()[0]?.searchParams.get("per_page"));
	window.scrollTo({
		top: document.documentElement.scrollHeight,
		behavior: "instant",
	});
	await expect.poll(() => lists().length).toBe(2);

	// Another reader dismisses the first batch's alerts, and the page reads it again.
	gone = size;
	await client.refetchQueries({
		predicate: ({ queryKey }) => queryKey[1] === "alerts" && queryKey[5] === 0,
	});
	window.scrollTo({
		top: document.documentElement.scrollHeight,
		behavior: "instant",
	});
	await expect.poll(() => lists().length).toBe(4);
	expect(lists()[3]?.searchParams.get("offset")).toBe(String(size));
});
