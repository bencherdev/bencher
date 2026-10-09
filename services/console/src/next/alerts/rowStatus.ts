import { useQueryClient } from "@tanstack/solid-query";
import { batch, createSignal } from "solid-js";
import type {
	JsonConsoleAlerts,
	JsonConsoleProject,
	JsonUpdateAlerts,
} from "../../types/bencher";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { type AlertLine, withStatuses } from "./lines";
import {
	type Changes,
	type Settable,
	begin,
	inView,
	linesInView,
	rollback,
	settle,
	shownDelta,
	statusOf,
	updateAlerts,
	withPending,
} from "./status";

/** What the row controls need of the page that lists the alerts. */
interface RowPage {
	/** The query key every cached batch of the page's alerts starts with. */
	cached: () => readonly unknown[];
	/** Reads the current view again from its first batch, after a change the API made smaller. */
	reread: () => Promise<void>;
	/** The query key of the current view's first batch, whose counts the page shows. */
	first: () => readonly unknown[];
	/** The current view without its status: a row changed under it stays under any status. */
	filters: () => string;
	/** The rows as shown, in order. */
	shown: () => readonly AlertLine[];
	/** The table that holds the rows. */
	table: () => HTMLElement | undefined;
	/** Where the focus goes once the last row leaves. */
	empty: () => HTMLElement | null | undefined;
	/** Hands each written batch the rows its old one drew, so they keep their objects and the focus. */
	keep: (from: JsonConsoleAlerts, to: JsonConsoleAlerts) => void;
}

/**
 * Dismiss and Reactivate on one alert row at a time, for a page that lists
 * alerts beside the Alerts page, as its rows behave: shown at once and put back
 * if the API refuses or changes nothing, kept where they were under the filters
 * they changed under, written into the page's cached batches once the API takes
 * them with the counts moved in the view's first batch, and the focus handed to
 * the row now in a row's place when it leaves, or to `empty` when none is left.
 */
export const useRowStatus = (page: RowPage) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const [changes, setChanges] = createSignal<Changes>(new Map());
	/** Why the last change did not go through. */
	const [notice, setNotice] = createSignal<"refused" | "partial">();
	const moveBadge = (delta: number) => {
		if (delta !== 0) {
			client.setQueryData<JsonConsoleProject>(
				consoleProjectQuery(api, slug()).queryKey,
				(project) =>
					project && {
						...project,
						active_alerts: Math.max(0, project.active_alerts + delta),
					},
			);
		}
	};
	/** What the API took, written into every cached batch of the page's alerts and `moved` into the counts of `first`; each reads again on its next visit. */
	const write = (
		statuses: ReadonlyMap<string, Settable>,
		first: readonly unknown[],
		moved: number,
	) => {
		const modified = Date.now();
		for (const [key, alerts] of client.getQueriesData<JsonConsoleAlerts>({
			queryKey: page.cached(),
		})) {
			if (!alerts) {
				continue;
			}
			let written = withStatuses(alerts, statuses, modified);
			if (
				key.length === first.length &&
				key.every((part, index) => part === first[index])
			) {
				written = { ...written, counts: withPending(written.counts, [moved]) };
			}
			if (written !== alerts) {
				client.setQueryData(key, written);
				const stored = client.getQueryData<JsonConsoleAlerts>(key);
				if (stored) {
					page.keep(alerts, stored);
				}
			}
		}
		client.invalidateQueries({
			predicate: ({ queryKey: [scope, kind, project] }) =>
				scope === "console" &&
				project === slug() &&
				(kind === "alerts" ||
					kind === "report" ||
					kind === "reports" ||
					kind === "threshold" ||
					kind === "thresholds"),
			refetchType: "none",
		});
	};

	/** The row's expand control, or once the row is gone the expand control of the row now in its place. */
	const nearestRow = (focused: Element | null) => {
		const row = focused?.closest("tr[aria-rowindex]");
		const index = Number(row?.getAttribute("aria-rowindex"));
		const control = (each: Element | undefined) =>
			each?.querySelector<HTMLElement>(".lr-twist");
		return () => {
			if (!row || row.isConnected) {
				return control(row ?? undefined);
			}
			const rows = [
				...(page.table()?.querySelectorAll("tbody tr[aria-rowindex]") ?? []),
			].filter((each) => each.isConnected && control(each));
			return (
				control(
					rows.find(
						(each) => Number(each.getAttribute("aria-rowindex")) >= index,
					) ?? rows.at(-1),
				) ?? page.empty()
			);
		};
	};
	/** The focused control, if the rows hold it, and where the focus goes if it leaves with its row. */
	const focusInRows = () => {
		const focused = document.activeElement;
		return focused instanceof HTMLElement && page.table()?.contains(focused)
			? { focused, fallback: nearestRow(focused) }
			: undefined;
	};
	const refocus = (held: ReturnType<typeof focusInRows>) => {
		const now = document.activeElement;
		if (
			held &&
			!held.focused.isConnected &&
			(now === null || now === document.body)
		) {
			held.fallback()?.focus();
		}
	};
	/** Make `update`; if it takes the focused control away with its row, the focus moves to the row now in its place. */
	const holdFocus = (update: () => void) => {
		const held = focusInRows();
		// The cache tells its readers as it is written, so they wait for `keep`.
		batch(update);
		// After a row's own controls hand the focus on.
		queueMicrotask(() => refocus(held));
	};
	/** Read the view again, holding the focus as `holdFocus` does. */
	const reread = async () => {
		// After the focus a change just handed on.
		await new Promise<void>((resolve) => queueMicrotask(resolve));
		const held = focusInRows();
		await page.reread();
		// The cache tells its readers on a later task.
		await new Promise((resolve) => setTimeout(resolve));
		refocus(held);
	};

	let requests = 0;
	const [inFlight, setInFlight] = createSignal(0);
	// A change the API made smaller than asked reads the project's count again once nothing is in flight.
	let recount = false;
	const change = async (status: Settable, line: AlertLine) => {
		const request = ++requests;
		// The view the change is made in, whatever the reader turns to before the API answers.
		const first = page.first();
		const order = page.shown();
		const at = order.findIndex(({ key }) => key === line.key);
		holdFocus(() => {
			const before = shownDelta(changes());
			setNotice(undefined);
			setChanges(
				begin(
					changes(),
					request,
					status,
					[{ line, after: at > 0 ? order[at - 1]?.key : undefined }],
					page.filters(),
				),
			);
			moveBadge(shownDelta(changes()) - before);
		});
		setInFlight(inFlight() + 1);
		try {
			const changed = await updateAlerts(api, slug(), {
				status: status as JsonUpdateAlerts["status"],
				alerts: [line.key],
			});
			const moved = changed < 1 ? 0 : status === "dismissed" ? -1 : 1;
			holdFocus(() => {
				const was = shownDelta(changes());
				if (moved === 0) {
					setChanges(rollback(changes(), request));
					setNotice("partial");
				} else {
					write(new Map([[line.key, status]]), first, moved);
					setChanges(settle(changes(), request));
				}
				moveBadge(shownDelta(changes()) + moved - was);
			});
			if (moved === 0) {
				recount = true;
				void reread();
			}
		} catch {
			holdFocus(() => {
				const was = shownDelta(changes());
				setChanges(rollback(changes(), request));
				moveBadge(shownDelta(changes()) - was);
				setNotice("refused");
			});
		} finally {
			setInFlight(inFlight() - 1);
			if (inFlight() === 0 && recount) {
				recount = false;
				client.invalidateQueries({
					queryKey: consoleProjectQuery(api, slug()).queryKey,
				});
			}
		}
	};
	return {
		/** The status a row shows: the page's newest change, else the API's. */
		statusOf: (line: AlertLine) => statusOf(line, changes()),
		/** The row's status was changed on this page. */
		changed: (line: AlertLine) => changes().has(line.key),
		/** The rows changed under the current filters, each with the row it followed. */
		kept: () =>
			[...changes().values()].filter(
				(change) => change.view === page.filters(),
			),
		/** The row was changed under the current filters. */
		keptHere: (line: AlertLine) =>
			changes().get(line.key)?.view === page.filters(),
		/** How far the rows move the active count past what the API took. */
		pending: () => shownDelta(changes()),
		/** The changes still out. */
		inFlight,
		/** The loaded rows still in `view` on the server. */
		inView: (
			lines: readonly AlertLine[],
			view: "active" | "dismissed" | "all",
		) => inView(lines, view, changes()),
		/** The lines still in `view` on the server. */
		linesInView: (
			lines: readonly AlertLine[],
			view: "active" | "dismissed" | "all",
		) => linesInView(lines, view, changes()),
		change,
		notice,
	};
};
