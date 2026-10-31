import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../plot/plot.css";
import "../report/report.css";
import "../thresholds/thresholds.css";
import "../alerts/alerts.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { type QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import {
	AlertStatus,
	type JsonConsoleAlertDetail,
	type JsonConsoleProject,
} from "../../types/bencher";
import { ALERT, REPORT, THRESHOLD, alertDetail, uuid } from "../alert/testing";
import { type Api, ApiError, type Method } from "../api";
import { createQueryClient } from "../cache";
import { PROJECTS_PATH } from "../paths";
import { rowsOf, tokenRgb } from "../plot/pixels";
import { ProjectContext } from "../project";
import { consoleProjectQuery } from "../queries";
import { decodeQuery } from "../query/query";
import { bootstrapOf } from "../settings/testing";
import { STRESS } from "../ui/testing";
import Alert from "./Alert";

const PATH = `${PROJECTS_PATH}/hashbrown/alerts/${ALERT}`;
const NAME = "blake3 input=64KiB simd=avx2 threads=1 Latency";
const ALERT_PATH = `/v0/console/projects/hashbrown/alerts/${ALERT}`;

let dispose: (() => void) | undefined;

interface Sent {
	method: Method;
	path: string;
	body: unknown;
}

/** An API that answers the alert with `read` and each change with `change`, recording both. */
const fakeApi = (
	read: (path: string) => unknown | Promise<unknown>,
	change: (sent: Sent) => Promise<{ changed: number }> = async () => ({
		changed: 1,
	}),
) => {
	const reads: string[] = [];
	const sent: Sent[] = [];
	const api: Api = {
		get: async <T,>(path: string) => {
			reads.push(path);
			return { data: (await read(path)) as T, headers: new Headers() };
		},
		send: async <T,>(method: Method, path: string, body?: unknown) => {
			const each = { method, path, body };
			sent.push(each);
			return { data: (await change(each)) as T, headers: new Headers() };
		},
	};
	return { api, reads, sent };
};

/** The alert at `ALERT_PATH`, and the shell's project with `bootstrap`; anything else never answers. */
const answers =
	(
		detail: JsonConsoleAlertDetail | Promise<never> = alertDetail(),
		bootstrap: JsonConsoleProject = { ...bootstrapOf(), active_alerts: 3 },
	) =>
	(path: string) =>
		path === ALERT_PATH
			? detail
			: path === "/v0/console/projects/hashbrown"
				? bootstrap
				: new Promise<never>(() => {});

const newClient = () => {
	const client = createQueryClient(() => {});
	client.setDefaultOptions({
		queries: { ...client.getDefaultOptions().queries, retry: false },
	});
	return client;
};

const mount = (api: Api, path = PATH, client = newClient()) => {
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
					<Route path="/:project/alerts/:alert" component={Alert} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
	return client;
};

const badge = (client: QueryClient) =>
	client.getQueryData<JsonConsoleProject>(
		consoleProjectQuery({} as Api, "hashbrown").queryKey,
	)?.active_alerts;
const heading = () => page.getByRole("heading", { level: 1 });
const dismiss = () =>
	page.getByRole("button", { name: `Dismiss the alert on ${NAME}` });
const reactivate = () =>
	page.getByRole("button", { name: `Reactivate the alert on ${NAME}` });
/** Whether the status pill draws the alert as no longer active. */
const muted = () =>
	document.querySelector(".rp-alerts")?.classList.contains("off");
/** When the pill says the alert was dismissed, if it names a date. */
const dismissedAt = () =>
	document.querySelector(".rp-alerts time")?.getAttribute("datetime");
/** The rows of `token`'s color down the alert's point, the plot's last. */
const markerRows = async (token: string) => {
	const area = page.getByRole("group", { name: /lines?,/ });
	await expect.element(area).toBeInTheDocument();
	const over = area.element() as HTMLElement;
	const canvas = over.parentElement?.querySelector("canvas");
	if (!canvas) {
		throw new Error("The plot area has no canvas");
	}
	const box = over.getBoundingClientRect();
	return rowsOf(canvas, box.right - 4, box.top, box.bottom, tokenRgb(token));
};

beforeEach(async () => {
	await page.viewport(1280, 860);
	window.scrollTo(0, 0);
});

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

// Kills an alert read in more than one request, or one that waits on the shell's.
test("reads the alert in one request beside the shell's", async () => {
	const { api, reads } = fakeApi(answers());
	mount(api);
	await expect.element(heading()).toHaveTextContent("blake3");
	expect(reads.filter((path) => path === ALERT_PATH)).toHaveLength(1);
	expect(reads).toContain("/v0/console/projects/hashbrown");
});

// Kills a page drawn before the reader's permissions arrive, whose action
// would then push the plot down once they did.
test("draws the alert only once the reader's permissions are known", async () => {
	let permit: ((project: JsonConsoleProject) => void) | undefined;
	const bootstrap = new Promise<JsonConsoleProject>((resolve) => {
		permit = resolve;
	});
	const { api } = fakeApi((path) =>
		path === ALERT_PATH ? alertDetail() : bootstrap,
	);
	mount(api);
	await expect.element(page.getByText("Loading the alert")).toBeInTheDocument();
	await new Promise((resolve) => setTimeout(resolve, 50));
	expect(document.querySelector("h1")).toBeNull();
	permit?.({ ...bootstrapOf(), active_alerts: 1 });
	await expect.element(dismiss()).toBeVisible();
});

// Kills a Dismiss that waits on the API, sends another alert or status, moves
// no count, swaps its button (dropping the focus), or leaves the plot's marker
// or the status pill as they were; and a Reactivate that does not undo it.
test("Dismiss and Reactivate change the alert at once, keeping the focus", async () => {
	const answered: (() => void)[] = [];
	const { api, sent } = fakeApi(
		answers(),
		() =>
			new Promise((resolve) => {
				answered.push(() => resolve({ changed: 1 }));
			}),
	);
	const client = mount(api);
	await expect.element(dismiss()).toBeVisible();
	expect(muted()).toBe(false);
	await expect
		.poll(async () => (await markerRows("--color-marker")).length)
		.toBeGreaterThan(0);

	await dismiss().click();
	await expect.element(reactivate()).toHaveFocus();
	expect(muted()).toBe(true);
	expect(dismissedAt()).toBeUndefined();
	await expect
		.poll(async () => (await markerRows("--color-marker")).length)
		.toBe(0);
	expect(await markerRows("--color-text-muted")).not.toHaveLength(0);
	await expect.poll(() => badge(client)).toBe(2);
	await expect.poll(() => sent).toHaveLength(1);
	expect(sent[0]).toEqual({
		method: "PATCH",
		path: "/v0/console/projects/hashbrown/alerts",
		body: { status: "dismissed", alerts: [ALERT] },
	});
	answered.shift()?.();
	await expect
		.poll(
			() =>
				client.getQueryData<JsonConsoleAlertDetail>([
					"console",
					"alert",
					"hashbrown",
					ALERT,
				])?.status,
		)
		.toBe("dismissed");

	await reactivate().click();
	await expect.element(dismiss()).toHaveFocus();
	expect(muted()).toBe(false);
	await expect
		.poll(async () => (await markerRows("--color-marker")).length)
		.toBeGreaterThan(0);
	await expect.poll(() => badge(client)).toBe(3);
	await expect.poll(() => sent).toHaveLength(2);
	expect(sent[1]?.body).toEqual({ status: "active", alerts: [ALERT] });
});

// Kills a refusal that leaves the alert, or the count, as if it had changed, and one that says nothing.
test("a refused Dismiss puts the alert and the count back, and says so", async () => {
	let refuse: (() => void) | undefined;
	const { api } = fakeApi(
		answers(),
		() =>
			new Promise((_, reject) => {
				refuse = () => reject(new ApiError(500, "server", "{}"));
			}),
	);
	const client = mount(api);
	await dismiss().click();
	await expect.element(reactivate()).toBeVisible();
	await expect.poll(() => badge(client)).toBe(2);
	await expect.poll(() => refuse).toBeDefined();
	expect(page.getByRole("alert").elements()).toHaveLength(0);
	refuse?.();
	await expect.element(page.getByRole("alert")).toBeVisible();
	await expect.element(dismiss()).toBeVisible();
	expect(muted()).toBe(false);
	expect(badge(client)).toBe(3);
});

// Kills a dismissed alert offered Dismiss, or one that does not say when.
test("a dismissed alert says when, and offers Reactivate", async () => {
	const { api } = fakeApi(
		answers(
			alertDetail({
				status: AlertStatus.Dismissed,
				modified: Date.UTC(2026, 9, 13, 9),
			}),
		),
	);
	mount(api);
	await expect.element(reactivate()).toBeVisible();
	expect(muted()).toBe(true);
	expect(dismissedAt()).toBe(new Date(Date.UTC(2026, 9, 13, 9)).toISOString());
	expect(dismiss().elements()).toHaveLength(0);
});

// Kills a silenced alert that offers a change the API never makes, or reads
// as active or as dismissed.
test("a silenced alert reads as no longer active, and offers nothing", async () => {
	const { api } = fakeApi(
		answers(alertDetail({ status: AlertStatus.Silenced })),
	);
	mount(api);
	await expect.element(heading()).toBeVisible();
	expect(muted()).toBe(true);
	expect(dismissedAt()).toBeUndefined();
	expect(
		page.getByRole("button", { name: /the alert on/ }).elements(),
	).toHaveLength(0);
});

// Kills a viewer offered a change the API refuses, or not told the page is read only.
test("a reader who cannot edit reads it, with no action", async () => {
	const { api } = fakeApi(
		answers(alertDetail(), {
			...bootstrapOf({ edit: false }),
			active_alerts: 1,
		}),
	);
	mount(api);
	await expect.element(page.getByText("Read only")).toBeVisible();
	await expect.element(heading()).toBeVisible();
	expect(
		page.getByRole("button", { name: /the alert on/ }).elements(),
	).toHaveLength(0);
});

// Kills a report, threshold, or Explore link to another place, Explore drawing
// the branch's current head or another window, and a crossed limit named by
// the wrong side.
test("the facts link to the report, the threshold, and the line in Explore", async () => {
	const { api } = fakeApi(answers());
	mount(api);
	await expect
		.element(page.getByRole("link", { name: /open the report$/ }))
		.toHaveAttribute("href", `${PROJECTS_PATH}/hashbrown/reports/${REPORT}`);
	await expect
		.element(
			page.getByRole("link", {
				name: "t_test · upper_boundary 0.99, open the threshold",
			}),
		)
		.toHaveAttribute(
			"href",
			`${PROJECTS_PATH}/hashbrown/thresholds/${THRESHOLD}`,
		);
	expect(
		[...document.querySelectorAll(".ap-fact dt")].map(
			(term) => term.textContent,
		),
	).toContain("Upper limit");

	const explore = page.getByRole("link", { name: `Open in Explore: ${NAME}` });
	await expect.element(explore).toBeVisible();
	const href = new URL(
		explore.element().getAttribute("href") ?? "",
		"https://bencher.dev",
	);
	expect(href.pathname).toBe(`${PROJECTS_PATH}/hashbrown/explore`);
	const query = decodeQuery(href.search);
	const detail = alertDetail();
	expect(query.branches).toEqual([{ uuid: uuid(20), head: uuid(21) }]);
	expect(query.report).toBe(REPORT);
	expect(query.window).toEqual({
		seconds: 28 * 86_400,
		end: detail.perf.window.end_time,
	});
});

// Kills a plot that opens with nothing pinned, and its own readout offering
// the alert the page already shows or a second Dismiss.
test("the plot opens pinned on the alert, its readout with neither the alert's link nor Dismiss", async () => {
	const { api } = fakeApi(answers());
	mount(api);
	const readout = page.getByRole("status");
	await expect.element(readout).toHaveTextContent("20.62 µs");
	await expect.element(readout).toHaveAttribute("aria-live", "polite");
	expect(
		readout.getByRole("link", { name: /the alert/ }).elements(),
	).toHaveLength(0);
	expect(
		readout.getByRole("button", { name: /the alert/ }).elements(),
	).toHaveLength(0);
});

// Kills a page that asks the API for an alert no UUID names, and an unknown alert drawn as a failure.
test("a malformed or unknown alert is not found", async () => {
	const { api, reads } = fakeApi(answers());
	mount(api, `${PROJECTS_PATH}/hashbrown/alerts/not-a-uuid`);
	await expect.element(heading()).toHaveTextContent("Alert not found");
	expect(reads.filter((path) => path.includes("/alerts/"))).toHaveLength(0);

	dispose?.();
	document.body.replaceChildren();
	const missing = fakeApi(
		answers(Promise.reject(new ApiError(404, "not_found", "{}"))),
	);
	mount(missing.api);
	await expect.element(heading()).toHaveTextContent("Alert not found");
});

// Kills a failed read that shows nothing or no heading, or a Retry that does not ask again.
test("an alert the API did not answer offers Retry, which loads it", async () => {
	let fail = true;
	const { api, reads } = fakeApi((path) =>
		path === ALERT_PATH
			? fail
				? Promise.reject(new ApiError(503, "server", "down"))
				: alertDetail()
			: { ...bootstrapOf(), active_alerts: 1 },
	);
	mount(api);
	const banner = page.getByRole("alert");
	await expect.element(banner).toBeVisible();
	await expect.element(heading()).toBeVisible();
	fail = false;
	await banner.getByRole("button", { name: "Retry" }).click();
	await expect.element(heading()).toHaveTextContent("blake3");
	expect(reads.filter((path) => path === ALERT_PATH)).toHaveLength(2);
});

// Kills a phone action that is narrower than the page or under 44 px, and facts that do not fold.
test("on a phone the action is a full width 44 px target, and the facts fold into rows", async () => {
	await page.viewport(390, 844);
	const { api } = fakeApi(answers());
	mount(api);
	await expect.element(dismiss()).toBeVisible();
	const button = dismiss().element().getBoundingClientRect();
	const head = document.querySelector(".rp-head")?.getBoundingClientRect();
	expect(button.height).toBeGreaterThanOrEqual(44);
	expect(Math.round(button.width)).toBe(Math.round(head?.width ?? 0));
	const facts = [...document.querySelectorAll(".ap-fact")].map((fact) =>
		fact.getBoundingClientRect(),
	);
	expect(new Set(facts.map(({ left }) => Math.round(left))).size).toBe(1);
	for (const fact of facts) {
		expect(fact.height).toBeGreaterThanOrEqual(44);
	}
});

// Kills facts that take the width of the widest one on a phone, which scrolls
// every alert's page sideways, and a title or sub line whose long names do.
test("on a phone the page never scrolls sideways, with long names or without", async () => {
	const long = alertDetail();
	const perf = long.perf as unknown as {
		branches: { name: string }[];
		testbeds: { name: string }[];
		benchmarks: { name: string }[];
		variants: { parameters: Record<string, string | number> }[];
	};
	(perf.branches[0] as { name: string }).name = STRESS.branch;
	(perf.testbeds[0] as { name: string }).name = STRESS.name;
	(perf.benchmarks[0] as { name: string }).name =
		`my_crate::${"module_000::".repeat(80)}bench_fn_name_0002`;
	(perf.variants[0] as { parameters: Record<string, string> }).parameters = {
		[`k${"k".repeat(63)}`]: "v".repeat(64),
	};
	for (const detail of [alertDetail(), long]) {
		for (const width of [320, 390]) {
			await page.viewport(width, 844);
			mount(fakeApi(answers(detail)).api);
			await expect
				.element(page.getByRole("region", { name: "The alert" }))
				.toBeVisible();
			expect(document.documentElement.scrollWidth).toBe(width);
			const facts = page.getByRole("region", { name: "The alert" }).element();
			for (const fact of facts.querySelectorAll(".ap-fact")) {
				expect(fact.getBoundingClientRect().right).toBeLessThanOrEqual(
					facts.getBoundingClientRect().right,
				);
			}
			dispose?.();
			dispose = undefined;
			document.body.replaceChildren();
		}
	}
});
