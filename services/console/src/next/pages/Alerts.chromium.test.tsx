import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "../report/report.css";
import "../reports/reports.css";
import "../alerts/alerts.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import {
	type QueryClient,
	QueryClientProvider,
	focusManager,
	onlineManager,
} from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonConsoleAlerts,
	JsonConsoleProject,
} from "../../types/bencher";
import { perfHolding, perfStatusOf } from "../alert/testing";
import { type Api, ApiError, type Method } from "../api";
import { READ, alertId, alertsFixture } from "../alerts/testing";
import { budget } from "../budget";
import { createQueryClient } from "../cache";
import { PROJECTS_PATH } from "../paths";
import { ProjectContext } from "../project";
import { consoleProjectQuery } from "../queries";
import { bootstrapOf } from "../settings/testing";
import { STRESS } from "../ui/testing";
import { linesDrawn, readLinesTopToBottom } from "../report/testing";
import Alerts from "./Alerts";

const PATH = `${PROJECTS_PATH}/hashbrown/alerts`;
// What a row's controls call its line: its benchmark, the one thing the list's lines differ in.
const NAME = "blake3";
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
			if (!url.pathname.endsWith("/v0/console/projects/hashbrown/alerts")) {
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
		reads.filter((url) =>
			url.pathname.endsWith("/v0/console/projects/hashbrown/alerts"),
		);
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
					base={PROJECTS_PATH}
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
		.toHaveTextContent("The alerts did not change");
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
	const listed = reads.find((url) =>
		url.pathname.endsWith("/v0/console/projects/hashbrown/alerts"),
	);
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/console/projects/hashbrown/alerts",
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

// Kills a group header that still draws Open report or Dismiss group beside
// its checkbox.
test("a group header's only control is its checkbox", async () => {
	const { api } = fakeApi(three);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const header = page
		.getByRole("checkbox", { name: `Select ${ALL}` })
		.element()
		.closest("tr");
	expect(
		[...(header?.querySelectorAll("a, button, input") ?? [])].map((control) =>
			control.getAttribute("aria-label"),
		),
	).toEqual([`Select ${ALL}`]);
});

// Kills a checked group that selects beyond its report, and a bulk Dismiss
// that sends other alerts than the selected ones.
test("a checked group's bulk Dismiss sends exactly that report's alerts", async () => {
	const { api, sent } = fakeApi(() =>
		alertsFixture([
			{ report: 0, alerts: [{ alert: 1 }, { alert: 2 }, { alert: 3 }] },
			{ report: 1, hours: 1, alerts: [{ alert: 4 }] },
		]),
	);
	mount(api, { ...bootstrapOf(), active_alerts: 4 });
	await expect.element(heading()).toHaveTextContent("4 active");
	await page.getByRole("checkbox", { name: `Select ${ALL}` }).click();
	await page
		.getByRole("button", { name: "Dismiss 3 selected active alerts" })
		.click();
	await expect.element(heading()).toHaveTextContent("1 active");
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/console/projects/hashbrown/alerts",
			body: {
				status: "dismissed",
				alerts: [alertId(1), alertId(2), alertId(3)],
			},
		},
	]);
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
		.element(page.getByText("Read only", { exact: true }))
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
		reads.filter((url) =>
			url.pathname.endsWith("/v0/console/projects/hashbrown/alerts"),
		);
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

const SHA256 = "sha256";
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

// Kills a Dismiss all that selects by the new search's filters while the rows
// on screen are still the last search's.
test("while a new search loads, Dismiss all waits", async () => {
	const { api } = fakeApi((url) =>
		url.searchParams.get("status") === "all"
			? new Promise<never>(() => {})
			: three(),
	);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const all = page.getByRole("button", { name: /^Dismiss all / });
	await expect.element(all).toBeEnabled();
	await status("All").click();
	await expect.element(all).toBeDisabled();
});

/** Select every alert of the report `ALL` names and dismiss them from the selection bar. */
const dismissGroup = async () => {
	await page.getByRole("checkbox", { name: `Select ${ALL}` }).click();
	await page
		.getByRole("button", { name: "Dismiss 3 selected active alerts" })
		.click();
};

// Kills a change the API made for fewer alerts than the page loaded that
// still shows every row changed, says nothing, or never reads the list again.
test("a change the API made for fewer alerts than were loaded puts the rows back, says so, and reads the list again", async () => {
	const { api, lists } = fakeApi(three, async () => ({ changed: 1 }));
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissGroup();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Some alerts did not change");
	await expect
		.poll(() => rowStates())
		.toEqual(["Dismiss", "Dismiss", "Dismiss"]);
	await expect.poll(() => lists().length).toBe(2);
});

// Kills a short change that leaves the tab badge and the project's count as
// the page guessed them, rather than as the API holds them.
test("the project's count is read again after a short change", async () => {
	const { api, reads, answered } = fakeApi(three, async () => ({ changed: 1 }));
	mount(api);
	const project = () =>
		reads.filter(
			({ pathname }) => pathname === "/v0/console/projects/hashbrown",
		).length;
	await expect.element(heading()).toHaveTextContent("3 active");
	expect(project()).toBe(0);
	await dismissGroup();
	await expect.poll(answered).toBe(1);
	await expect.poll(project).toBe(1);
});

// Kills a recount read while another change is still out, before the API
// has taken it, so the badge jumps to a count the page then moves again.
test("the project's count is read again only once no change is in flight", async () => {
	const answers: ((changed: number) => void)[] = [];
	const { api, reads, answered } = fakeApi(
		three,
		() =>
			new Promise((resolve) => {
				answers.push((changed) => resolve({ changed }));
			}),
	);
	mount(api);
	const project = () =>
		reads.filter(
			({ pathname }) => pathname === "/v0/console/projects/hashbrown",
		).length;
	await expect.element(heading()).toHaveTextContent("3 active");
	await press(dismissButtons().nth(0));
	await press(dismissButtons().nth(0));
	await expect.poll(() => answers.length).toBe(2);
	answers[0]?.(0);
	await expect.poll(answered).toBe(1);
	await new Promise((resolve) => setTimeout(resolve, 100));
	expect(project()).toBe(0);
	answers[1]?.(1);
	await expect.poll(answered).toBe(2);
	await expect.poll(project).toBe(1);
});

// Kills a reread that drops the focus with a row it leaves out.
test("a reread that leaves out the focused row moves the focus to the row now in its place", async () => {
	let read = 0;
	const { api, answered } = fakeApi(
		() =>
			read++ === 0
				? three()
				: alertsFixture([{ report: 0, alerts: [{ alert: 1 }, { alert: 3 }] }]),
		async () => ({ changed: 0 }),
	);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const rows = () =>
		[...document.querySelectorAll("tbody tr[aria-rowindex]")].filter((row) =>
			row.querySelector("td.lr-act"),
		);
	await press(dismissButtons().nth(1));
	await expect.poll(answered).toBe(1);
	await expect.poll(() => rows().length).toBe(2);
	await expect.poll(focusedName).toBe(`Select ${NAME}`);
	expect(document.activeElement?.closest("tr")).toBe(rows()[1]);
});

// Kills a reread on returning to the window that draws the rows anew, which
// drops the focus and redraws every open plot. The reread answers as the API
// does under Active: the dismissed alert left out and its report counted one
// less.
test("returning to the window after a change reads the list again and keeps its rows, plots, and focus", async () => {
	let read = 0;
	const { api, answered, lists } = fakeApi(() =>
		read++ === 0
			? three()
			: alertsFixture([{ report: 0, alerts: [{ alert: 1 }, { alert: 3 }] }]),
	);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	await page
		.getByRole("button", { name: `Expand ${NAME}` })
		.first()
		.click();
	const plot = page.getByRole("region", { name: `${NAME}, full plot` });
	await expect.element(plot).toBeVisible();
	const drawn = plot.element();
	const row = dismissButtons().nth(1).element().closest("tr");
	await press(dismissButtons().nth(1));
	await expect.poll(answered).toBe(1);
	await expect.poll(focusedName).toBe(`Reactivate the alert on ${NAME}`);
	focusManager.setFocused(false);
	focusManager.setFocused(true);
	try {
		await expect.poll(() => lists().length).toBe(2);
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(focusedName()).toBe(`Reactivate the alert on ${NAME}`);
		expect(document.activeElement?.closest("tr")).toBe(row);
		expect(plot.element()).toBe(drawn);
	} finally {
		focusManager.setFocused(undefined);
	}
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

// Kills a checked group's Dismiss dropping the focus once the group has no
// active row, and a header drawn anew on the change.
test("after a checked group's Dismiss the focus rests on the count and the header stays", async () => {
	const { api, answered } = fakeApi(three, async () => ({ changed: 3 }));
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const group = page.getByRole("checkbox", { name: `Select ${ALL}` });
	const header = group.element().closest("tr");
	await group.click();
	await press(
		page.getByRole("button", { name: "Dismiss 3 selected active alerts" }),
	);
	await expect.poll(answered).toBe(1);
	await new Promise((resolve) => requestAnimationFrame(resolve));
	expect(focusedName()).toBe("3 selected");
	expect(group.element().closest("tr")).toBe(header);
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
test("on a tablet the rows fold, and a folded Reactivate leaves the focus on its row", async () => {
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

// Kills a group header laid out at another height than it is drawn, so the
// rows below it drift from where the list puts them.
test.each([
	[1280, 44],
	[900, 64],
])(
	"at %i px a group header is laid out at the height it is drawn",
	async (width, line) => {
		await page.viewport(width, 400);
		const reports = 30;
		const { api } = fakeApi(() =>
			alertsFixture(
				Array.from({ length: reports }, (_, report) => ({
					report,
					hours: report,
					alerts: [{ alert: report + 1 }],
				})),
			),
		);
		mount(api, { ...bootstrapOf(), active_alerts: reports });
		await expect.element(heading()).toHaveTextContent(`${reports} active`);
		await new Promise((resolve) => requestAnimationFrame(resolve));
		const body = document.querySelector("tbody") as HTMLElement;
		expect(body.getBoundingClientRect().height).toBe(reports * (44 + line));
	},
);

// Kills a report, run, or threshold page, or the Runs list, left showing a
// status the API no longer holds, and another project's pages read again.
test("a confirmed change has the report, run, Runs, and threshold pages read again", async () => {
	const { api, answered } = fakeApi(three);
	const client = mount(api);
	const kinds = ["report", "reports", "run", "runs", "threshold", "thresholds"];
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
		.toEqual(kinds.map(() => true));
	expect(
		client.getQueryState(["console", "report", "other", "x"])?.isInvalidated,
	).toBe(false);
});

// Kills a checked group's Dismiss that reaches the report's alerts not
// loaded, which the reader never selected, or whose count waits for the API.
test("a checked group's Dismiss takes only its loaded alerts out of the count, at once", async () => {
	const { api, sent } = fakeApi(
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
	await dismissGroup();
	await expect.element(heading()).toHaveTextContent("7 active");
	expect(sent.map(({ body }) => body)).toEqual([
		{ status: "dismissed", alerts: [alertId(1), alertId(2), alertId(3)] },
	]);
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
	await linesDrawn(document);
	expect(lists()).toHaveLength(1);
	answer?.();
	await expect.poll(() => lists().length).toBe(2);
	expect(lists()[1]?.searchParams.get("offset")).toBe(String(size - 1));
});

/** How many alerts a newer report can raise ahead of the rest. */
const ARRIVALS = 4;

/**
 * The page's API over the alerts numbered 1 to `total`, each on a benchmark of
 * its own and active until it is dismissed, the list served from its `offset`;
 * `holdList` keeps a list from `offset` out, read once it resolves, and
 * `holdAnswer` keeps one out once it was read. A change counts only the alerts
 * it dismissed, `elsewhere` dismisses an alert as another reader would, and
 * `arrive` has a newer report raise alerts numbered past `total`, listed ahead
 * of the rest.
 */
const dismissibleAlerts = (
	total: number,
	holdList: (offset: number) => Promise<void> | undefined,
	holdAnswer: (offset: number) => Promise<void> | undefined = () => undefined,
) => {
	const dismissed = new Set<number>();
	const arrived: number[] = [];
	const benchmarks = Array.from({ length: total + ARRIVALS }, (_, index) => ({
		uuid: `bench-${index + 1}-uuid`,
		name: `bench-${index + 1}`,
		slug: `bench-${index + 1}`,
	}));
	const variants = benchmarks.map(({ uuid }, index) => ({
		uuid: `variant-${uuid}`,
		benchmark: index,
		parameters: { n: 0 },
	}));
	const api = fakeApi(
		async (url) => {
			const offset = Number(url.searchParams.get("offset"));
			const size = Number(url.searchParams.get("per_page"));
			await holdList(offset);
			const ids = [
				...arrived,
				...Array.from({ length: total }, (_, index) => index + 1),
			].filter(
				(id) =>
					url.searchParams.get("status") !== "active" || !dismissed.has(id),
			);
			const batch = ids.slice(offset, offset + size);
			const reports = [
				{ report: 1, raised: (id: number) => id > total },
				{ report: 0, raised: (id: number) => id <= total },
			];
			const answer = {
				...alertsFixture(
					reports
						.filter(({ raised }) => batch.some(raised))
						.map(({ report, raised }) => ({
							report,
							total: ids.filter(raised).length,
							alerts: batch
								.filter(raised)
								.map((alert) => ({ alert, benchmark: alert - 1 })),
						})),
					{ benchmarks, variants } as Partial<JsonConsoleAlerts>,
				),
				total: ids.length,
				counts: {
					active: total + arrived.length - dismissed.size,
					dismissed: dismissed.size,
					silenced: 0,
				},
				read_time: Date.now(),
			} as JsonConsoleAlerts;
			await holdAnswer(offset);
			return answer;
		},
		async (sent) => {
			const { alerts = [] } = sent.body as { alerts?: string[] };
			const before = dismissed.size;
			for (const uuid of alerts) {
				dismissed.add(Number(uuid.slice(-11)));
			}
			return { changed: dismissed.size - before };
		},
	);
	return {
		...api,
		elsewhere: (alert: number) => dismissed.add(alert),
		arrive: (...alerts: number[]) => arrived.unshift(...alerts),
	};
};
const STATUS_BUTTON =
	"button[aria-label^='Dismiss the alert on '], button[aria-label^='Reactivate the alert on ']";
/** Each alert row's name, top to bottom, read a screen at a time. */
const alertNamesTopToBottom = async () => {
	const names = new Map<number, string>();
	await readLinesTopToBottom(document, (rows) => {
		for (const row of rows) {
			const name = row
				.querySelector(STATUS_BUTTON)
				?.getAttribute("aria-label")
				?.replace(/^(Dismiss|Reactivate) the alert on /, "");
			if (name) {
				names.set(Number(row.getAttribute("aria-rowindex")), name);
			}
		}
	});
	return [...names].sort(([a], [b]) => a - b).map(([, name]) => name);
};

// Kills a next batch the API read after a dismissal it took while that batch
// was out, which starts an alert late and is never asked for again from the
// recounted offset, so that alert is never drawn.
test("a dismissal the API takes while the next batch is out skips and repeats no alert", async () => {
	await page.viewport(1280, 400);
	const total = 40;
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const { api, lists, answered } = dismissibleAlerts(total, (offset) =>
		offset > 0 ? held : undefined,
	);
	mount(api, { ...bootstrapOf(), active_alerts: total });
	await expect.element(heading()).toHaveTextContent(`${total} active`);
	const size = Number(lists()[0]?.searchParams.get("per_page"));
	const toBottom = () =>
		window.scrollTo({
			top: document.documentElement.scrollHeight,
			behavior: "instant",
		});
	toBottom();
	await expect.poll(() => lists().length).toBe(2);
	window.scrollTo({ top: 0, behavior: "instant" });
	await page
		.getByRole("button", { name: "Dismiss the alert on bench-1", exact: true })
		.click();
	await expect.poll(answered).toBe(1);
	release();
	// Asked for again with the reader still at the top, where nothing nears the end.
	await expect.poll(() => lists().length).toBe(3);

	await expect
		.poll(
			() => {
				toBottom();
				return [...document.querySelectorAll(STATUS_BUTTON)]
					.at(-1)
					?.getAttribute("aria-label");
			},
			{ timeout: 5_000 },
		)
		.toBe(`Dismiss the alert on bench-${total}`);
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(await alertNamesTopToBottom()).toEqual(
		Array.from({ length: total }, (_, index) => `bench-${index + 1}`),
	);
	expect(
		lists()
			.slice(0, 3)
			.map((url) => url.searchParams.get("offset")),
	).toEqual(["0", String(size), String(size - 1)]);
});

const toEnd = () =>
	window.scrollTo({
		top: document.documentElement.scrollHeight,
		behavior: "instant",
	});
const offsetsOf = (urls: URL[]) =>
	urls.map((url) => url.searchParams.get("offset"));
/** The names of the alerts numbered `alerts`, top to bottom, once the last one is drawn. */
const drawnOnce = async (alerts: number[]) => {
	await expect
		.poll(
			() => {
				toEnd();
				return [...document.querySelectorAll(STATUS_BUTTON)]
					.at(-1)
					?.getAttribute("aria-label")
					?.replace(/^(Dismiss|Reactivate) the alert on /, "");
			},
			{ timeout: 5_000 },
		)
		.toBe(`bench-${alerts.at(-1)}`);
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(await alertNamesTopToBottom()).toEqual(
		alerts.map((alert) => `bench-${alert}`),
	);
};
const numbered = (from: number, to: number) =>
	Array.from({ length: to - from + 1 }, (_, index) => from + index);

// Kills a later batch read again on returning to the window or the network,
// from the offset it was first asked from, which the dismissals ahead of it
// have moved: only the first batch is read again, and the next batch counts
// from it.
test("returning to the window or the network after dismissals reads only the first batch again and draws every alert once", async () => {
	await page.viewport(1280, 400);
	const total = 60;
	const { api, lists, answered } = dismissibleAlerts(total, () => undefined);
	mount(api, { ...bootstrapOf(), active_alerts: total });
	await expect.element(heading()).toHaveTextContent(`${total} active`);
	const size = Number(lists()[0]?.searchParams.get("per_page"));
	await expect
		.poll(
			() => {
				toEnd();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	window.scrollTo({ top: 0, behavior: "instant" });
	for (const alert of [1, 2, 3]) {
		await page
			.getByRole("button", {
				name: `Dismiss the alert on bench-${alert}`,
				exact: true,
			})
			.click();
	}
	await expect.poll(answered).toBe(3);
	const before = lists().length;
	focusManager.setFocused(false);
	focusManager.setFocused(true);
	try {
		await expect.poll(() => lists().length).toBeGreaterThan(before);
		await new Promise((resolve) => setTimeout(resolve, 200));
		expect(offsetsOf(lists().slice(before))).toEqual(["0"]);
	} finally {
		focusManager.setFocused(undefined);
	}
	await page
		.getByRole("button", { name: "Dismiss the alert on bench-4", exact: true })
		.click();
	await expect.poll(answered).toBe(4);
	const back = lists().length;
	onlineManager.setOnline(false);
	onlineManager.setOnline(true);
	await expect.poll(() => lists().length).toBeGreaterThan(back);
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(offsetsOf(lists().slice(back))).toEqual(["0"]);

	await drawnOnce(numbered(1, total));
	expect(offsetsOf(lists().slice(back + 1))).toEqual([String(3 * size - 4)]);
});

// Kills a reread after a short change that reads the later batches again from
// the offsets they were first asked from, or before the first batch answers:
// each is asked for again from the offset the batches before it now count.
test("a reread after a short change asks each later batch again from the offset the first batch's answer counts", async () => {
	await page.viewport(1280, 400);
	const total = 60;
	let holding = false;
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const { api, lists, answered, elsewhere } = dismissibleAlerts(
		total,
		(offset) => (holding && offset === 0 ? held : undefined),
	);
	mount(api, { ...bootstrapOf(), active_alerts: total });
	await expect.element(heading()).toHaveTextContent(`${total} active`);
	const size = Number(lists()[0]?.searchParams.get("per_page"));
	for (const alert of [1, 2, 3]) {
		await page
			.getByRole("button", {
				name: `Dismiss the alert on bench-${alert}`,
				exact: true,
			})
			.click();
	}
	await expect.poll(answered).toBe(3);
	await expect
		.poll(
			() => {
				toEnd();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	expect(offsetsOf(lists())).toEqual([
		"0",
		String(size - 3),
		String(2 * size - 3),
	]);
	window.scrollTo({ top: 0, behavior: "instant" });

	// Another reader dismissed bench-5 first, so this dismissal changes nothing.
	elsewhere(5);
	holding = true;
	const before = lists().length;
	await page
		.getByRole("button", { name: "Dismiss the alert on bench-5", exact: true })
		.click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Some alerts did not change");
	await expect.poll(() => lists().length).toBeGreaterThan(before);
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(offsetsOf(lists().slice(before))).toEqual(["0"]);
	holding = false;
	release();
	await expect.poll(() => lists().length).toBe(before + 2);
	expect(offsetsOf(lists().slice(before + 1))).toEqual([String(size)]);

	await drawnOnce([1, 2, 3, 4, ...numbered(6, total)]);
});

// Kills a reread whose first batch failed that still sets the later batches
// aside, which asks them again from offsets counted over rows the API no
// longer holds as the page shows them.
test("a reread whose first batch fails keeps the later batches as they were", async () => {
	await page.viewport(1280, 400);
	const total = 60;
	let failing = false;
	const { api, lists, elsewhere } = dismissibleAlerts(total, (offset) =>
		failing && offset === 0
			? Promise.reject(new ApiError(500, "server", "{}"))
			: undefined,
	);
	mount(api, { ...bootstrapOf(), active_alerts: total });
	await expect.element(heading()).toHaveTextContent(`${total} active`);
	const size = Number(lists()[0]?.searchParams.get("per_page"));
	await expect
		.poll(
			() => {
				toEnd();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	window.scrollTo({ top: 0, behavior: "instant" });
	const table = page.getByRole("table", {
		name: "Alerts, grouped by the report that raised them",
	});
	const rows = String(2 + 3 * size);
	await expect.element(table).toHaveAttribute("aria-rowcount", rows);

	elsewhere(5);
	failing = true;
	const before = lists().length;
	await page
		.getByRole("button", { name: "Dismiss the alert on bench-5", exact: true })
		.click();
	await expect.poll(() => lists().length).toBeGreaterThan(before);
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(offsetsOf(lists().slice(before))).toEqual(["0"]);
	await expect.element(table).toHaveAttribute("aria-rowcount", rows);
});

/** The reader was away long enough for every batch of the list to go stale. */
const away = (client: QueryClient) =>
	client.invalidateQueries({
		queryKey: ["console", "alerts", "hashbrown"],
		refetchType: "none",
	});
/** Load the first three batches of a list longer than them, then scroll back to the top. */
const threeBatches = async (lists: () => URL[]) => {
	await expect
		.poll(
			() => {
				toEnd();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(3);
	window.scrollTo({ top: 0, behavior: "instant" });
};
/** Return to the window, and the offsets asked for once `count` batches were. */
const refocusAsking = async (lists: () => URL[], count: number) => {
	const before = lists().length;
	focusManager.setFocused(false);
	focusManager.setFocused(true);
	try {
		await expect
			.poll(() => lists().length)
			.toBeGreaterThanOrEqual(before + count);
		await new Promise((resolve) => setTimeout(resolve, 200));
		return offsetsOf(lists().slice(before));
	} finally {
		focusManager.setFocused(undefined);
	}
};

// Kills a first batch read again on returning to the window whose fresh
// answer pushes its last rows past its end while the later batches keep their
// old answers, so no batch draws the rows pushed out: the list reads itself
// again from the top, the next batch from the offset the fresh first batch
// counts.
test("alerts raised ahead while the reader was away push no alert out of the list on returning to the window", async () => {
	await page.viewport(1280, 400);
	const total = 60;
	const { api, lists, arrive } = dismissibleAlerts(total, () => undefined);
	const client = mount(api, { ...bootstrapOf(), active_alerts: total });
	await expect.element(heading()).toHaveTextContent(`${total} active`);
	const size = Number(lists()[0]?.searchParams.get("per_page"));
	await threeBatches(lists);

	arrive(61, 62);
	await away(client);
	expect(await refocusAsking(lists, 2)).toEqual(["0", String(size)]);
	await drawnOnce([61, 62, ...numbered(1, total)]);
});

// Kills a list read again from the top whenever its first batch's fresh
// answer falls short of the next batch, as it does when nothing changed
// ahead; whenever that answer leaves out a row dismissed here, which left the
// view; or whenever it leaves out rows it held though it still reaches the
// next batch: alerts another reader dismissed ahead only leave the list.
test("returning to the window reads only the first batch again when nothing ahead pushes its rows past the next batch", async () => {
	await page.viewport(1280, 400);
	const total = 60;
	const { api, lists, answered, arrive, elsewhere } = dismissibleAlerts(
		total,
		() => undefined,
	);
	const client = mount(api, { ...bootstrapOf(), active_alerts: total });
	await expect.element(heading()).toHaveTextContent(`${total} active`);
	await threeBatches(lists);

	await away(client);
	expect(await refocusAsking(lists, 1)).toEqual(["0"]);
	await page
		.getByRole("button", { name: "Dismiss the alert on bench-5", exact: true })
		.click();
	await expect.poll(answered).toBe(1);
	arrive(61);
	await away(client);
	expect(await refocusAsking(lists, 1)).toEqual(["0"]);
	elsewhere(7);
	elsewhere(8);
	await away(client);
	expect(await refocusAsking(lists, 1)).toEqual(["0"]);
	await drawnOnce([61, ...numbered(1, 6), ...numbered(9, total)]);
});

// Kills a next batch still on its way when the first batch's fresh answer
// pushed rows past its end, which then draws what it read before the alerts
// were raised and leaves the rows pushed out undrawn: it is asked again from
// the offset the fresh first batch counts.
test("a next batch read before alerts were raised ahead is asked again once the first batch's fresh answer pushes rows past its end", async () => {
	await page.viewport(1280, 400);
	const total = 60;
	let holding = false;
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const { api, lists, arrive } = dismissibleAlerts(
		total,
		() => undefined,
		(offset) => (holding && offset > 0 ? held : undefined),
	);
	const client = mount(api, { ...bootstrapOf(), active_alerts: total });
	await expect.element(heading()).toHaveTextContent(`${total} active`);
	const size = Number(lists()[0]?.searchParams.get("per_page"));
	holding = true;
	await expect
		.poll(
			() => {
				toEnd();
				return lists().length;
			},
			{ timeout: 5_000 },
		)
		.toBe(2);
	holding = false;
	window.scrollTo({ top: 0, behavior: "instant" });

	arrive(61, 62);
	await away(client);
	expect(await refocusAsking(lists, 2)).toEqual(["0", String(size)]);
	release();
	await drawnOnce([61, 62, ...numbered(1, total)]);
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

// Kills a checked group's bulk Dismiss offered on a report none of whose rows
// still alert.
test("a checked report with no active row offers no Dismiss, only Reactivate", async () => {
	const { api } = fakeApi(() =>
		alertsFixture([
			{ report: 0, alerts: [{ alert: 1, status: "dismissed" }] },
			{ report: 1, hours: 1, alerts: [{ alert: 2 }] },
		]),
	);
	mount(api, { ...bootstrapOf(), active_alerts: 1 }, `${PATH}?status=all`);
	await page.getByRole("checkbox", { name: `Select ${ALL}` }).click();
	await expect
		.element(
			page.getByRole("button", {
				name: "Reactivate 1 selected dismissed alert",
			}),
		)
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: "Dismiss", exact: true }))
		.toBeDisabled();
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
		.element(page.getByText("9 active in the project", { exact: true }))
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

// Kills an alert row whose Explore link opens another line or window than
// Open 1 does for that row alone, and row links that wait for a selection.
test("an alert row's Open in Explore is Open 1 for that row alone", async () => {
	const { api } = fakeApi(three);
	mount(api);
	const table = page.getByRole("table", {
		name: "Alerts, grouped by the report that raised them",
	});
	await expect.element(table).toBeVisible();
	await userEvent.hover(table.getByRole("columnheader", { name: "Line" }));
	await userEvent.hover(table.getByRole("columnheader", { name: "Limit" }));
	const row = page
		.getByRole("link", { name: `Open in Explore: ${NAME}` })
		.first();
	await expect.element(row).toBeVisible();
	const href = row.element().getAttribute("href");
	expect(href).toMatch(/^\/console\/projects\/hashbrown\/explore\?/);
	await page
		.getByRole("checkbox", { name: `Select ${NAME}` })
		.first()
		.click();
	await expect
		.element(page.getByRole("link", { name: "Open 1 in Explore" }))
		.toHaveAttribute("href", href as string);
});

/** Pins the last point of the plot in `region`, as the keys do. */
const pinLast = async (region: ReturnType<typeof page.getByRole>) => {
	const plot = region.getByRole("group", { name: /^1 line/ });
	await expect.element(plot).toBeVisible();
	(plot.element() as HTMLElement).focus();
	await userEvent.keyboard("{End}");
};

// Kills a readout on a row's alert that changes it outside its row, which
// leaves the row, the count, and the badge as they were, and one that does not
// link to the alert's page.
test("a pinned readout on a row's alert opens it and dismisses it as its row does", async () => {
	const { api, sent } = fakeApi(three);
	const client = mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const row = dismissButtons().nth(1).element().closest("tr");
	await page
		.getByRole("button", { name: `Expand ${NAME}` })
		.nth(1)
		.click();
	const region = page.getByRole("region", { name: `${NAME}, full plot` });
	await pinLast(region);
	const readout = region.getByRole("status");
	await expect
		.element(readout.getByRole("link", { name: `Alert: ${NAME}` }))
		.toHaveAttribute("href", `${PROJECTS_PATH}/hashbrown/alerts/${alertId(2)}`);
	await readout
		.getByRole("button", { name: `Dismiss the alert on ${NAME}` })
		.click();
	await expect.element(heading()).toHaveTextContent("2 active");
	expect(badge(client)).toBe(2);
	expect(row?.textContent).toContain("Dismissed just now");
	await expect
		.element(
			readout.getByRole("button", { name: `Reactivate the alert on ${NAME}` }),
		)
		.toBeVisible();
	expect(sent).toEqual([
		{
			method: "PATCH",
			path: "/v0/console/projects/hashbrown/alerts",
			body: { status: "dismissed", alerts: [alertId(2)] },
		},
	]);
});

// Kills a change on the list that leaves the other answers holding the alert,
// such as Explore's, with its old status for a return to them.
test("a change on the list is written into the other answers that hold the alert", async () => {
	const { api } = fakeApi(three);
	const bootstrap = { ...bootstrapOf(), active_alerts: 3 };
	const client = newClient(api, bootstrap);
	const perf = ["console", "perf", "hashbrown", "q"];
	client.setQueryData(perf, perfHolding(alertId(2)));
	mount(api, bootstrap, PATH, client);
	await expect.element(heading()).toHaveTextContent("3 active");
	await dismissButtons().nth(1).click();
	await expect.element(heading()).toHaveTextContent("2 active");
	await expect
		.poll(() => perfStatusOf(client.getQueryData(perf)))
		.toBe("dismissed");
});

// Kills a viewer offered a Dismiss in a pinned readout, which the API refuses.
test("a viewer's pinned readout links the alert and offers no change", async () => {
	const { api } = fakeApi(three);
	mount(api, { ...bootstrapOf({ edit: false }), active_alerts: 3 });
	await expect.element(heading()).toHaveTextContent("3 active");
	await page
		.getByRole("button", { name: `Expand ${NAME}` })
		.nth(1)
		.click();
	const region = page.getByRole("region", { name: `${NAME}, full plot` });
	await pinLast(region);
	const readout = region.getByRole("status");
	await expect
		.element(readout.getByRole("link", { name: `Alert: ${NAME}` }))
		.toBeVisible();
	expect(
		readout.getByRole("button", { name: /the alert on/ }).elements(),
	).toHaveLength(0);
});

// Kills a row naming an alert whose name opens nothing, or another alert.
test("each row's name links to its alert's page", async () => {
	const { api } = fakeApi(three);
	mount(api);
	await expect.element(heading()).toHaveTextContent("3 active");
	const links = page.getByRole("link", {
		name: "blake3 n=0 Latency, open the alert",
	});
	await expect.element(links.first()).toBeVisible();
	expect(links.elements().map((link) => link.getAttribute("href"))).toEqual(
		[1, 2, 3].map((n) => `${PROJECTS_PATH}/hashbrown/alerts/${alertId(n)}`),
	);
});

// Kills a report header whose testbed is cut at its end, or loses its share of
// the line to a long branch.
test("a report header keeps its testbed's end beside a long branch", async () => {
	const leaf = "runner-ubuntu-latest-large";
	for (const width of [1440, 390]) {
		await page.viewport(width, 800);
		const list = alertsFixture([{ report: 0, alerts: [{ alert: 1 }] }]);
		const { api } = fakeApi(() => ({
			...list,
			branches: list.branches.map((branch, index) =>
				index === 0 ? { ...branch, name: STRESS.branch } : branch,
			),
			testbeds: list.testbeds.map((testbed, index) =>
				index === 0
					? { ...testbed, name: `ci/${"linux/".repeat(30)}${leaf}` }
					: testbed,
			),
		}));
		mount(api);
		await expect.poll(() => document.querySelector(".al-gname")).not.toBeNull();
		const [branch, testbed] = document.querySelectorAll<HTMLElement>(
			".al-gname .ui-trunc",
		);
		expect(branch?.dataset.tail).toBe("very-long-leaf");
		expect(testbed?.dataset.tail).toBe(leaf);
		expect(testbed?.getBoundingClientRect().width).toBeGreaterThanOrEqual(
			(branch?.getBoundingClientRect().width ?? 0) - 0.5,
		);
		expect(document.documentElement.scrollWidth).toBe(width);
		dispose?.();
		dispose = undefined;
		document.body.replaceChildren();
	}
});
