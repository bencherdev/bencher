import { useLocation, useNavigate } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Dialog from "@bencherdev/ui/Dialog";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import {
	type QueryKey,
	type QueryObserverResult,
	useQueryClient,
} from "@tanstack/solid-query";
import {
	type Accessor,
	For,
	Match,
	Show,
	Switch,
	batch,
	createComputed,
	createEffect,
	createMemo,
	createSignal,
	mapArray,
	on,
} from "solid-js";
import type {
	JsonConsoleAlerts,
	JsonConsoleProject,
	JsonUpdateAlerts,
} from "../../types/bencher";
import AlertActions from "../alerts/AlertActions";
import Controls, { type FilterNames } from "../alerts/Controls";
import { useFold } from "../alerts/fold";
import {
	type AlertsBatch,
	alertsBatch,
	alertsQuery,
	nameQuery,
	searchKey,
	updateAlerts,
} from "../alerts/query";
import {
	type AlertGroup,
	type AlertLine,
	alertLinesOf,
	alertSlots,
	distinctLines,
	groupState,
	toggleGroup,
	visibleLines,
	withKept,
	withStatuses,
} from "../alerts/rows";
import {
	type AlertsSearch,
	type Filter,
	decodeSearch,
	dismissAllFilter,
	encodeSearch,
	filterCount,
	historyDays,
} from "../alerts/search";
import {
	type Changes,
	type Settable,
	begin,
	changeable,
	inChunks,
	inView,
	rollback,
	settle,
	shownDelta,
	statusOf,
	withPending,
} from "../alerts/status";
import { projectPath, reportPath } from "../paths";
import { formatWhen } from "../plot/format";
import { useNarrow } from "../plot/narrow";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import LineTable from "../report/LineTable";
import { type GroupRow, type ReportLine, plotOf } from "../report/lines";
import { lineLabel } from "../report/row";
import { WINDOWS, windowPhrase } from "../reports/search";

const SKELETON_ROWS = 8;
/** A report's header: its identity on one line when wide, with its controls on a second when narrow. */
const GROUP_HEIGHT = { wide: 44, narrow: 88 } as const;
const FILTERS: readonly Filter[] = ["branch", "testbed", "measure"];
/** The first batch of every search, one object so its query is kept, and re-keyed, rather than started again. */
const FIRST = { offset: 0, round: 0 } as const;

type Batch = QueryObserverResult<AlertsBatch>;

/** A project's alerts under the reports that raised them, triaged in place. */
const Alerts = () => {
	const { api, slug } = useProject();
	const location = useLocation();
	const navigate = useNavigate();
	const client = useQueryClient();
	const narrow = useNarrow();
	const fold = useFold();
	const search = createMemo(() =>
		decodeSearch(new URLSearchParams(location.search)),
	);
	const setSearch = (next: AlertsSearch) => {
		const query = encodeSearch(next);
		// Router paths are relative to the console's base.
		navigate(`/${slug()}/alerts${query ? `?${query}` : ""}`, {
			scroll: false,
		});
	};
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	const canEdit = () => bootstrap().data?.permissions.edit === true;
	const names = useNames(search);

	const perPage = alertsBatch();
	const searched = createMemo(() => encodeSearch(search()));
	// A rolling window is measured from when the reader picked it, so every batch binds the same bounds.
	const now = createMemo(on(searched, () => Date.now()));
	const [starts, setStarts] = createSignal<{ offset: number; round: number }[]>(
		[FIRST],
	);
	createComputed(on(searched, () => setStarts([FIRST]), { defer: true }));
	const queries = createMemo(
		mapArray(starts, (start) =>
			useQueryResult(() =>
				alertsQuery(api, slug(), search(), { ...start, perPage }, now()),
			),
		),
	);
	const batches = () => queries().map((batch) => batch());
	/** The batches loaded so far, up to the first one still loading or failed. */
	const loaded = createMemo(() => {
		const ready: Batch[] = [];
		for (const batch of batches()) {
			if (batch.data === undefined) {
				break;
			}
			ready.push(batch);
		}
		return ready;
	});
	/** The batch after the loaded ones, while it loads or once it failed. */
	const next = () => batches()[loaded().length];
	const first = () => loaded()[0]?.data;
	/** The rows on screen are the last search's, until the new one answers. */
	const busy = () => batches()[0]?.isPlaceholderData === true;

	// Each batch resolves its rows once, so the rows on screen keep their objects as data arrives.
	const resolved = new WeakMap<JsonConsoleAlerts, AlertLine[]>();
	const linesOfBatch = (alerts: JsonConsoleAlerts) => {
		let lines = resolved.get(alerts);
		if (!lines) {
			lines = alertLinesOf(alerts);
			resolved.set(alerts, lines);
		}
		return lines;
	};

	const [changes, setChanges] = createSignal<Changes>(new Map());
	/** The active count each request still out moves among alerts not loaded, as guessed when it was sent. */
	const [guesses, setGuesses] = createSignal<ReadonlyMap<number, number>>(
		new Map(),
	);
	const [pending, setPending] = createSignal(0);
	/** Why the last change did not all go through. */
	const [notice, setNotice] = createSignal<"refused" | "partial">();
	/** The search without its status: a row changed under these filters stays under any status. */
	const filters = createMemo(() =>
		encodeSearch({ ...search(), status: "active" }),
	);

	const listed = createMemo(() =>
		distinctLines(
			loaded().map((batch) =>
				batch.data ? linesOfBatch(batch.data.alerts) : [],
			),
		),
	);
	const shown = createMemo(() => {
		const current = changes();
		const kept = [...current.values()].filter(
			(change) => change.view === filters(),
		);
		return visibleLines(
			withKept(listed(), kept),
			search().status,
			(line) => statusOf(line, current),
			(line) => current.get(line.key)?.view === filters(),
		);
	});
	const layout = createMemo((previous?: ReturnType<typeof alertSlots>) =>
		alertSlots(shown(), previous?.groups),
	);
	const groupOf = createMemo(
		() => new Map(layout().groups.map((group) => [group.row, group])),
	);
	const metrics = createMemo(
		() => new Set(shown().map(({ metric }) => metric)).size > 1,
	);
	/** How far the page moves the active count past what the API took. */
	const adjustment = () =>
		shownDelta(changes()) +
		[...guesses().values()].reduce((sum, guess) => sum + guess, 0);
	const counts = createMemo(() => {
		const batch = first();
		return batch && withPending(batch.alerts.counts, [adjustment()]);
	});
	const activeCount = () => counts()?.active ?? 0;

	// The next batch starts after the loaded rows still in the view, since a row changed in place leaves it.
	const [wantMore, setWantMore] = createSignal(false);
	const nearEnd = () => {
		const last = loaded().at(-1)?.data?.alerts;
		if (!last || next() !== undefined) {
			return;
		}
		if (pending() > 0) {
			setWantMore(true);
			return;
		}
		const offset = inView(listed(), search().status, changes());
		const full =
			last.groups.reduce((sum, group) => sum + group.alerts.length, 0) ===
			perPage;
		if (full && offset < last.total) {
			const round = starts().filter((start) => start.offset === offset).length;
			setStarts([...starts(), { offset, round }]);
		}
	};
	createEffect(
		on(pending, (count) => {
			if (count === 0 && wantMore()) {
				setWantMore(false);
				nearEnd();
			}
		}),
	);

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
	const alertsKey = () => ["console", "alerts", slug()];
	/**
	 * What the API took, written into every alert batch held, and `moved` into
	 * the counts of `first`, the view the change was made in; each reads again on
	 * its next visit, and the rows on screen keep their objects.
	 */
	const write = (
		statuses: ReadonlyMap<string, Settable>,
		first: QueryKey,
		moved: number,
	) => {
		const modified = Date.now();
		for (const [key, batch] of client.getQueriesData<AlertsBatch>({
			queryKey: alertsKey(),
		})) {
			if (!batch) {
				continue;
			}
			let alerts = withStatuses(batch.alerts, statuses, modified);
			if (
				moved !== 0 &&
				key.length === first.length &&
				key.every((part, index) => part === first[index])
			) {
				alerts = { ...alerts, counts: withPending(alerts.counts, [moved]) };
			}
			if (alerts !== batch.alerts) {
				client.setQueryData<AlertsBatch>(key, { ...batch, alerts });
				const lines = resolved.get(batch.alerts);
				const stored = client.getQueryData<AlertsBatch>(key)?.alerts;
				if (lines && stored) {
					resolved.set(stored, lines);
				}
			}
		}
		client.invalidateQueries({ queryKey: alertsKey(), refetchType: "none" });
		client.invalidateQueries({
			predicate: ({ queryKey: [scope, kind, project] }) =>
				scope === "console" &&
				project === slug() &&
				(kind === "report" ||
					kind === "reports" ||
					kind === "threshold" ||
					kind === "thresholds"),
		});
	};

	let main: HTMLElement | undefined;
	let title: HTMLHeadingElement | undefined;
	let selection: HTMLElement | undefined;
	/** A row's checkbox, or the row's now in its place once it leaves; the selection count outside the rows. */
	const nearestRow = (focused: Element | null) => {
		const row = focused?.closest("tr[aria-rowindex]");
		const index = Number(row?.getAttribute("aria-rowindex"));
		return () => {
			if (row?.isConnected) {
				return row.querySelector<HTMLElement>("input");
			}
			const rows = [
				...(main?.querySelectorAll("tbody tr[aria-rowindex]") ?? []),
			].filter((each) => each.querySelector("input"));
			const near = row
				? (rows.find(
						(each) => Number(each.getAttribute("aria-rowindex")) >= index,
					) ?? rows.at(-1))
				: undefined;
			return near?.querySelector<HTMLElement>("input") ?? selection;
		};
	};
	/** Make `update`; if it takes away or disables the focused control, the focus moves to `target`. */
	const holdFocus = (
		update: () => void,
		target?: () => HTMLElement | null | undefined,
	) => {
		const focused = document.activeElement;
		const held = focused instanceof HTMLElement && main?.contains(focused);
		const fallback = target ?? nearestRow(focused);
		batch(update);
		// After a row's own controls hand the focus on.
		queueMicrotask(() => {
			const now = document.activeElement;
			if (
				held &&
				(!focused.isConnected || focused.matches(":disabled")) &&
				(now === null || now === document.body || now === focused)
			) {
				fallback()?.focus();
			}
		});
	};

	let requests = 0;
	/**
	 * Show `status` on `lines` at once and ask the API; a refusal, or a change of
	 * fewer than were loaded (the API does not say which), returns them to what it
	 * last took.
	 */
	const change = async (
		status: Settable,
		lines: readonly AlertLine[],
		body: JsonUpdateAlerts,
		{
			guess = 0,
			target,
		}: { guess?: number; target?: () => HTMLElement | null | undefined } = {},
	) => {
		const request = ++requests;
		// The view the change is made in, whatever the reader turns to before the API answers.
		const view = searchKey(slug(), search());
		const counted = alertsQuery(
			api,
			slug(),
			search(),
			{ ...FIRST, perPage },
			now(),
		).queryKey;
		const order = shown();
		const at = new Map(order.map((line, index) => [line.key, index]));
		holdFocus(() => {
			const before = adjustment();
			setNotice(undefined);
			setChanges(
				begin(
					changes(),
					request,
					status,
					lines.map((line) => ({
						line,
						after: order[(at.get(line.key) ?? 0) - 1]?.key,
					})),
					filters(),
				),
			);
			if (guess !== 0) {
				setGuesses(new Map([...guesses(), [request, guess]]));
			}
			moveBadge(adjustment() - before);
		}, target);
		setPending(pending() + 1);
		const unguess = () => {
			const rest = new Map(guesses());
			rest.delete(request);
			setGuesses(rest);
		};
		try {
			const changed = await updateAlerts(api, slug(), body);
			const moved = status === "dismissed" ? -changed : changed;
			const partial = changed < lines.length;
			holdFocus(() => {
				const was = adjustment();
				write(
					partial ? new Map() : new Map(lines.map(({ key }) => [key, status])),
					counted,
					moved,
				);
				setChanges((partial ? rollback : settle)(changes(), request));
				unguess();
				moveBadge(adjustment() + moved - was);
				if (partial) {
					setNotice("partial");
				}
			});
			if (partial) {
				client.invalidateQueries({ queryKey: view });
			}
		} catch {
			holdFocus(() => {
				const was = adjustment();
				setChanges(rollback(changes(), request));
				unguess();
				moveBadge(adjustment() - was);
				setNotice("refused");
			});
		} finally {
			setPending(pending() - 1);
		}
	};
	const changeListed = (
		status: Settable,
		lines: readonly AlertLine[],
		target?: () => HTMLElement | null | undefined,
	) => {
		for (const chunk of inChunks(changeable(lines, status, changes()))) {
			change(
				status,
				chunk,
				{
					status: status as JsonUpdateAlerts["status"],
					alerts: chunk.map(({ key }) => key),
				},
				target ? { target } : {},
			);
		}
	};
	/** The active alerts the first batch counted, up to when the API read it. */
	const dismissFilter = () => {
		const read = first();
		return (
			read && dismissAllFilter(search(), read.bounds, read.alerts.read_time)
		);
	};
	const dismissGroup = (group: AlertGroup) => {
		const filter = dismissFilter();
		if (!filter) {
			return;
		}
		const ofGroup = (line: AlertLine) => line.report.uuid === group.report.uuid;
		const lines = changeable(shown().filter(ofGroup), "dismissed", changes());
		const unseen = group.report.total - listed().filter(ofGroup).length;
		change(
			"dismissed",
			lines,
			{
				status: "dismissed" as JsonUpdateAlerts["status"],
				filter: { ...filter, reports: [group.report.uuid] },
			},
			{
				guess: search().status === "active" ? -Math.max(0, unseen) : 0,
			},
		);
	};
	const [confirming, setConfirming] = createSignal(false);
	const dismissAll = () => {
		setConfirming(false);
		const filter = dismissFilter();
		if (!filter) {
			return;
		}
		const lines = changeable(shown(), "dismissed", changes());
		change(
			"dismissed",
			lines,
			{ status: "dismissed" as JsonUpdateAlerts["status"], filter },
			{
				guess: -Math.max(0, activeCount() - lines.length),
				target: () => title,
			},
		);
	};
	const [selected, setSelected] = createSignal<ReadonlySet<string>>(new Set());
	const chosen = createMemo(() =>
		shown().filter(({ key }) => selected().has(key)),
	);
	// Explore's query codec loads with the first selection, not with the page.
	const [explorer, setExplorer] =
		createSignal<typeof import("../alerts/explore")["exploreSearchOf"]>();
	const choose = (next: ReadonlySet<string>) => {
		if (!explorer()) {
			import("../alerts/explore")
				.then(({ exploreSearchOf }) => setExplorer(() => exploreSearchOf))
				.catch(() => {});
		}
		setSelected(next);
	};
	const select = (key: string, on: boolean) => {
		const next = new Set(selected());
		if (on) {
			next.add(key);
		} else {
			next.delete(key);
		}
		choose(next);
	};
	const explore = () => {
		const searchOf = explorer();
		return searchOf && chosen().length > 0
			? `${projectPath(slug(), "explore")}${searchOf(chosen(), search().window)}`
			: undefined;
	};
	const [expanded, setExpanded] = createSignal<ReadonlySet<string>>(new Set());

	const history = () => {
		const window = search().window;
		const preset =
			window.kind === "rolling"
				? WINDOWS.find(({ days }) => days === window.days)?.label
				: undefined;
		return preset ?? (window.kind === "all" ? "3m" : `${historyDays(window)}d`);
	};
	const phrase = () => windowText(search(), now());
	const nameOf = (line: ReportLine) =>
		lineLabel(
			{
				benchmark: line.benchmark.name,
				parameters: line.parameters,
				measure: line.measure.name,
				metric: line.metric,
			},
			metrics(),
		).name;
	// Every row of this list is an alert's.
	const alertOf = (line: ReportLine) => line as AlertLine;
	const statusShown = (line: ReportLine) => statusOf(alertOf(line), changes());

	const GroupHeader = (props: {
		group: GroupRow;
		index: number;
		columns: number;
	}) => (
		<Show when={groupOf().get(props.group)}>
			{(group) => (
				<ReportHeader
					group={group()}
					index={props.index}
					columns={props.columns}
					narrow={fold()}
					canEdit={canEdit()}
					busy={busy()}
					active={shown().some(
						(line) =>
							line.report.uuid === group().report.uuid &&
							statusOf(line, changes()) === "active",
					)}
					state={groupState(group().keys, selected())}
					slug={slug()}
					onSelect={() => choose(toggleGroup(group().keys, selected()))}
					onDismiss={() => dismissGroup(group())}
				/>
			)}
		</Show>
	);

	return (
		<main
			class="page al-page"
			ref={(element) => {
				main = element;
			}}
		>
			<div class="pagehead al-head">
				<div class="crumb-line">Alerts</div>
				<div class="al-titlerow">
					<Heading
						level={1}
						size="xl"
						tabindex={-1}
						ref={(element: HTMLHeadingElement) => {
							title = element;
						}}
					>
						<Show
							when={counts()}
							fallback={
								<>
									<Skeleton size="text" class="al-title-skel" />
									<span class="sr-only">Loading the alerts</span>
								</>
							}
						>
							{(shownCounts) => <>{shownCounts().active} active</>}
						</Show>
					</Heading>
					{/* Drawn once the count is known, so its arrival moves nothing. */}
					<Show when={canEdit() && counts()}>
						<Button
							size="sm"
							class="al-all"
							aria-haspopup="dialog"
							disabled={activeCount() === 0 || busy()}
							aria-label={`Dismiss all ${activeCount()} active alerts that match the filters`}
							onClick={() => setConfirming(true)}
						>
							Dismiss all {activeCount()}
						</Button>
					</Show>
				</div>
				<p class="ph-sub al-sub">
					<Show
						when={counts()}
						fallback={<Skeleton size="text" class="al-sub-skel" />}
					>
						{(shownCounts) => (
							<>
								{filterCount(search()) > 0
									? `Filtered to ${filterNames(search(), names()).join(", ")} · ${bootstrap().data?.active_alerts ?? 0} active in the project`
									: `${shownCounts().dismissed} dismissed and ${shownCounts().silenced} silenced ${phrase().sentence}`}
							</>
						)}
					</Show>
				</p>
				<Show when={bootstrap().data?.permissions.edit === false}>
					<p class="ro-note">
						<Icon name="read-only" />
						Read only. Ask a project Maintainer for access.
					</p>
				</Show>
			</div>
			<Controls
				search={search()}
				names={names()}
				now={now()}
				onSearch={setSearch}
			/>
			<Switch>
				<Match when={notice() === "refused"}>
					<Banner status="error" role="alert" class="load-error">
						<span class="grow">
							The alerts did not change: the Bencher API did not answer.
						</span>
					</Banner>
				</Match>
				<Match when={notice() === "partial"}>
					<Banner status="warning" role="alert" class="load-error">
						<span class="grow">
							Some alerts did not change, so the list is read again.
						</span>
					</Banner>
				</Match>
			</Switch>
			<Selection
				chosen={chosen()}
				status={search().status}
				changes={changes()}
				canEdit={canEdit()}
				narrow={narrow()}
				explore={explore()}
				count={(element) => {
					selection = element;
				}}
				onClear={() => setSelected(new Set())}
				onChange={(status, lines) =>
					changeListed(status, lines, () => selection)
				}
			/>
			<Switch>
				<Match when={first() && shown().length === 0 && !busy()}>
					<Empty
						search={search()}
						quiet={(counts()?.dismissed ?? 0) + (counts()?.silenced ?? 0) === 0}
						window={phrase().sentence}
					/>
				</Match>
				<Match when={shown().length > 0}>
					<div class="al-scroll">
						<LineTable
							label="Alerts, grouped by the report that raised them"
							slots={layout().slots}
							rows={layout().slots.length}
							narrow={fold()}
							metrics={metrics()}
							history={`History, ${history()}`}
							expanded={expanded()}
							onExpand={(line, open) => {
								const next = new Set(expanded());
								if (open) {
									next.add(line.key);
								} else {
									next.delete(line.key);
								}
								setExpanded(next);
							}}
							selected={selected()}
							onSelect={(line, on) => select(line.key, on)}
							group={{ height: GROUP_HEIGHT, row: GroupHeader }}
							dimmed={(line) => statusShown(line) !== "active"}
							actions={{
								cell: (line) => (
									<AlertActions
										name={nameOf(line)}
										status={statusShown(line)}
										justNow={changes().has(line.key)}
										modified={alertOf(line).modified}
										onDismiss={
											canEdit()
												? () => changeListed("dismissed", [alertOf(line)])
												: undefined
										}
										onReactivate={
											canEdit()
												? () => changeListed("active", [alertOf(line)])
												: undefined
										}
									/>
								),
								narrow: (line) => statusShown(line) !== "active",
							}}
							plot={(line) => ({
								data: plotOf(line, line.branch.name, line.testbed.name),
								note: `${history()}, ending at its report`,
								reportHref: (uuid) => reportPath(slug(), uuid),
							})}
							onNearEnd={nearEnd}
							busy={busy()}
							loading={next()?.isFetching ?? false}
							onRetry={
								next()?.isLoadingError ? () => next()?.refetch() : undefined
							}
						/>
					</div>
				</Match>
				<Match when={batches()[0]?.isLoadingError}>
					<Banner status="error" role="alert" class="load-error">
						<span class="grow">
							Alerts did not load: the Bencher API did not answer.
						</span>
						<Button size="sm" onClick={() => batches()[0]?.refetch()}>
							Retry
						</Button>
					</Banner>
				</Match>
				<Match when={true}>
					<div class="lr-table al-skeleton" aria-busy="true">
						<span class="sr-only">Loading the alerts</span>
						<For each={Array.from({ length: SKELETON_ROWS })}>
							{() => <Skeleton size="text" class="al-skeleton-row" />}
						</For>
					</div>
				</Match>
			</Switch>
			<Show when={confirming()}>
				<DismissAll
					count={activeCount()}
					search={search()}
					names={names()}
					window={phrase().object}
					onCancel={() => setConfirming(false)}
					onConfirm={dismissAll}
				/>
			</Show>
		</main>
	);
};

export default Alerts;

const plural = (count: number, noun: string) =>
	`${count} ${count === 1 ? noun : `${noun}s`}`;

/** The window inside a sentence ("in the last 4 weeks") and on its own ("the last 4 weeks"). */
const windowText = (search: AlertsSearch, now: number) => {
	if (search.window.kind === "all") {
		return { sentence: "over all time", object: "all time" };
	}
	const phrase = windowPhrase(search.window, now);
	return {
		sentence: phrase.replace(/^(In|From)/, (word) => word.toLowerCase()),
		object: phrase.replace(/^(In|From) /, ""),
	};
};

/** The names the chips show for the filters set. */
const filterNames = (search: AlertsSearch, names: FilterNames) =>
	FILTERS.flatMap((filter) =>
		search[filter] === undefined ? [] : [names[filter] ?? "…"],
	);

/** The names of the branch, testbed, and measure the search filters by, by UUID. */
const useNames = (search: Accessor<AlertsSearch>) => {
	const { api, slug } = useProject();
	const branch = useQueryResult(() =>
		nameQuery(api, slug(), "branches", search().branch),
	);
	const testbed = useQueryResult(() =>
		nameQuery(api, slug(), "testbeds", search().testbed),
	);
	const measure = useQueryResult(() =>
		nameQuery(api, slug(), "measures", search().measure),
	);
	return (): FilterNames => ({
		branch: branch().data,
		testbed: testbed().data,
		measure: measure().data,
	});
};

/** A report's row in the list: its checkbox, identity, and controls. */
const ReportHeader = (props: {
	group: AlertGroup;
	index: number;
	columns: number;
	narrow: boolean;
	canEdit: boolean;
	/** The rows are the last search's while a new one loads, so Dismiss group would select by the wrong filters. */
	busy: boolean;
	/** Some row of it still alerts. */
	active: boolean;
	state: "none" | "some" | "all";
	slug: string;
	onSelect: () => void;
	onDismiss: () => void;
}) => {
	const where = () =>
		`${props.group.branch.name}, ${props.group.testbed.name}, ${formatWhen(props.group.report.start)}`;
	const meta = () =>
		[
			formatWhen(props.group.report.start),
			props.group.report.hash,
			props.group.report.adapter,
			plural(props.group.report.total, "alert"),
		]
			.filter(Boolean)
			.join(" · ");
	let box: HTMLInputElement | undefined;
	createEffect(() => {
		if (box) {
			box.indeterminate = props.state === "some";
		}
	});
	const controls = () => (
		<span class="al-gacts">
			<a
				class="lnk"
				href={reportPath(props.slug, props.group.report.uuid)}
				aria-label={`Open report on ${where()}`}
			>
				Open report
			</a>
			<Show when={props.canEdit && props.active}>
				<span class="dotsep" aria-hidden="true">
					·
				</span>
				<button
					type="button"
					class="lnk"
					disabled={props.busy}
					aria-label={`Dismiss group on ${where()}`}
					onClick={() => props.onDismiss()}
				>
					Dismiss group
				</button>
			</Show>
		</span>
	);
	return (
		<tr class="al-group" aria-rowindex={props.index}>
			<td colSpan={props.columns}>
				<div class="al-ghead">
					<label class="lr-hit">
						<input
							ref={box}
							type="checkbox"
							class="lr-cb"
							checked={props.state === "all"}
							aria-label={`Select the alerts in the report on ${where()}`}
							onChange={() => props.onSelect()}
						/>
					</label>
					<div class="al-gtitle">
						<span class="al-gname">
							<Show when={props.active}>
								<svg class="lr-alertdot" viewBox="0 0 8 8" aria-hidden="true">
									<circle cx="4" cy="4" r="4" />
								</svg>
							</Show>
							<b>
								{props.group.branch.name} · {props.group.testbed.name}
							</b>
						</span>
						<span class="al-gmeta">{meta()}</span>
					</div>
					<Show when={!props.narrow}>{controls()}</Show>
				</div>
				<Show when={props.narrow}>
					<div class="al-gfoot">{controls()}</div>
				</Show>
			</td>
		</tr>
	);
};

const Empty = (props: {
	search: AlertsSearch;
	/** Nothing in the window was dismissed or silenced either. */
	quiet: boolean;
	window: string;
}) => {
	const unfiltered = () =>
		props.search.status === "active" && filterCount(props.search) === 0;
	return (
		<div class="card soft al-empty">
			<Heading level={2} size="lg">
				{unfiltered() ? "No active alerts" : "No alerts match"}
			</Heading>
			<p>
				{unfiltered()
					? props.quiet
						? `Nothing alerted ${props.window}.`
						: `Every alert ${props.window} is dismissed. Dismissed and All still list them.`
					: `Nothing matches these filters ${props.window}. Clear a filter or widen the window.`}
			</p>
		</div>
	);
};

const Selection = (props: {
	chosen: readonly AlertLine[];
	status: AlertsSearch["status"];
	changes: Changes;
	canEdit: boolean;
	narrow: boolean;
	explore: string | undefined;
	/** The count, where the focus goes once a change takes away the control it was on. */
	count: (element: HTMLElement) => void;
	onClear: () => void;
	onChange: (status: Settable, lines: readonly AlertLine[]) => void;
}) => {
	const count = () => props.chosen.length;
	const dismissable = () =>
		changeable(props.chosen, "dismissed", props.changes);
	const reactivatable = () => changeable(props.chosen, "active", props.changes);
	return (
		<section class="rp-selbar" aria-label="Selected alerts">
			<span
				class="rp-selcount"
				aria-live="polite"
				tabindex={-1}
				ref={props.count}
			>
				{count() > 0
					? `${count()} selected`
					: props.narrow
						? "Select alerts"
						: "Select alerts to act on them together"}
			</span>
			<Show when={count() > 0}>
				<button
					type="button"
					class="lnk rp-clear"
					onClick={() => props.onClear()}
				>
					Clear
				</button>
			</Show>
			<span class="spacer" />
			<Show when={props.canEdit}>
				<Show
					when={dismissable().length > 0}
					fallback={
						<Button size="sm" disabled>
							Dismiss
						</Button>
					}
				>
					<Button
						size="sm"
						aria-label={`Dismiss ${plural(dismissable().length, "selected active alert")}`}
						onClick={() => props.onChange("dismissed", dismissable())}
					>
						Dismiss {dismissable().length}
					</Button>
				</Show>
				<Show when={props.status !== "active" && reactivatable().length > 0}>
					<Button
						size="sm"
						aria-label={`Reactivate ${plural(reactivatable().length, "selected dismissed alert")}`}
						onClick={() => props.onChange("active", reactivatable())}
					>
						Reactivate {reactivatable().length}
					</Button>
				</Show>
			</Show>
			<Show
				when={props.explore}
				fallback={
					<Button size="sm" variant="primary" disabled>
						Open in Explore
					</Button>
				}
			>
				{(href) => (
					<a
						class="ui-button"
						data-variant="primary"
						data-size="sm"
						href={href()}
						aria-label={`Open ${count()} in Explore`}
					>
						{props.narrow ? `Open ${count()}` : `Open ${count()} in Explore`}
					</a>
				)}
			</Show>
		</section>
	);
};

const DismissAll = (props: {
	count: number;
	search: AlertsSearch;
	names: FilterNames;
	window: string;
	onCancel: () => void;
	onConfirm: () => void;
}) => (
	<Dialog
		aria-labelledby="al-da-title"
		aria-describedby="al-da-desc"
		onDismiss={props.onCancel}
	>
		<div class="ui-dialog-head">
			<h2 id="al-da-title">Dismiss {plural(props.count, "active alert")}?</h2>
			<Button variant="ghost" aria-label="Close" onClick={props.onCancel}>
				<Icon name="close" />
			</Button>
		</div>
		<div class="ui-dialog-body">
			<p id="al-da-desc" class="text2">
				This dismisses every active alert that matches the filters on this page,
				including alerts not loaded yet.
			</p>
			<dl>
				<For each={FILTERS}>
					{(filter) => (
						<div class="kv">
							<dt>{filter}</dt>
							<dd>
								{props.search[filter] === undefined
									? "any"
									: (props.names[filter] ?? "…")}
							</dd>
						</div>
					)}
				</For>
				<div class="kv">
					<dt>window</dt>
					<dd>{props.window}</dd>
				</div>
			</dl>
			<p class="muted">
				Each moves to Dismissed, where Reactivate brings it back.
			</p>
		</div>
		<div class="ui-dialog-foot">
			<Button onClick={props.onCancel}>Cancel</Button>
			<Button variant="primary" onClick={props.onConfirm}>
				Dismiss {plural(props.count, "alert")}
			</Button>
		</div>
	</Dialog>
);
