import { useLocation, useNavigate } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Chip from "@bencherdev/ui/Chip";
import Sheet from "@bencherdev/ui/Sheet";
import { useQueryClient } from "@tanstack/solid-query";
import {
	For,
	Show,
	createEffect,
	createMemo,
	createSignal,
	onCleanup,
	onMount,
} from "solid-js";
import type {
	JsonConsoleLatestReport,
	JsonConsolePerf,
	JsonPlot,
} from "../../types/bencher";
import { ApiError } from "../api";
import type { Option } from "../explore/Box";
import Controls from "../explore/Controls";
import { plotData } from "../explore/data";
import { withValue } from "../explore/edit";
import Editor from "../explore/Editor";
import Header, { type Confirmation } from "../explore/Header";
import { measuresLayout } from "../explore/layout";
import Leave from "../explore/Leave";
import PlotFrame from "../explore/PlotFrame";
import {
	newPlot,
	pinPlot,
	pinTitle,
	plotPatch,
	queryFromPlot,
	shownAmong,
	unpinPlot,
	unsaved,
} from "../explore/pinned";
import {
	latestQuery,
	nameQuery,
	perfQuery,
	plotKey,
	plotQuery,
	variantsQuery,
} from "../explore/queries";
import { drawable, drawsPlot, requestKey } from "../explore/request";
import { useRequested } from "../explore/requested";
import Start from "../explore/Start";
import { chipSummary } from "../explore/summary";
import { rollingName } from "../explore/window";
import {
	NEXT_PROJECTS,
	TABS,
	parseNextPath,
	projectPath,
	reportPath,
	tabOf,
} from "../paths";
import { formatDate } from "../plot/format";
import { useNarrow } from "../plot/narrow";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import { exploreSearchFromClassic } from "../query/classic";
import { lineVisible, settleVisibility } from "../query/line";
import {
	type ExploreQuery,
	MAX_ENTRIES,
	type Parameters,
	decodeQuery,
	encodeQuery,
} from "../query/query";

const DIMENSIONS = [
	["branches", "branches"],
	["testbeds", "testbeds"],
	["benchmarks", "benchmarks"],
	["sets", "parameters"],
	["measures", "measures"],
	["metrics", "metrics"],
] as const;

/** The editor for an unsaved plot, and for a pinned one; the query is the URL. */
const Explore = (props: { readOnly?: boolean }) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const location = useLocation();
	const navigate = useNavigate();
	const narrow = useNarrow();
	const query = createMemo(() => decodeQuery(location.search));
	// Router paths are relative to the console's base.
	const here = () => location.pathname.slice(NEXT_PROJECTS.length);
	// A view change keeps where the query came from; an edit makes it the reader's own.
	const go = (next: ExploreQuery, replace = false) =>
		navigate(`${here()}${encodeQuery(next)}`, {
			replace,
			scroll: false,
			...(replace ? { state: location.state } : {}),
		});
	const project = () => `/v0/projects/${encodeURIComponent(slug())}`;

	// A classic perf link, or one edited by hand, settles into Explore's spelling.
	onMount(() => {
		const canonical = exploreSearchFromClassic(location.search);
		if (canonical !== location.search) {
			navigate(`${here()}${canonical}`, {
				replace: true,
				scroll: false,
			});
		}
	});

	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	const canPin = () =>
		!props.readOnly && bootstrap().data?.permissions.create === true;

	const { requested, pending } = useRequested(
		query,
		requestKey,
		(next) =>
			client.getQueryData(perfQuery(api, slug(), next).queryKey) !== undefined,
	);
	const perf = useQueryResult(() => ({
		...perfQuery(api, slug(), requested()),
		enabled: drawable(requested()),
		// The lines drawn stay, dimmed, until the next answer replaces them.
		placeholderData: (
			previous: JsonConsolePerf | undefined,
			last: { queryKey: readonly unknown[] } | undefined,
		) => (last?.queryKey[2] === slug() ? previous : undefined),
	}));
	// The last answer drawn stays until another arrives, through a failed one too.
	let kept: { slug: string; perf: JsonConsolePerf } | undefined;
	// The same answer keeps its identity, so a change to the view alone redraws nothing new.
	const answer = createMemo(() => {
		if (!drawable(query())) {
			return undefined;
		}
		const { data } = perf();
		if (data !== undefined) {
			kept = { slug: slug(), perf: data };
			return data;
		}
		return kept?.slug === slug() ? kept.perf : undefined;
	});
	const data = createMemo(() => {
		const current = answer();
		return current && plotData(current);
	});
	const busy = () =>
		answer() !== undefined &&
		(perf().isPlaceholderData ||
			pending() ||
			(perf().data === undefined && perf().isFetching));
	const refused = () => {
		const { isLoadingError, error } = perf();
		if (!isLoadingError) {
			return undefined;
		}
		return error instanceof ApiError &&
			error.kind !== "network" &&
			error.kind !== "server"
			? "The API refused this query."
			: "The API did not answer.";
	};
	const [PlotView, setPlotView] = createSignal(plotView);
	const wantPlot = () => {
		if (!PlotView()) {
			loadPlot()
				.then((view) => setPlotView(() => view))
				.catch(() => {});
		}
	};
	createEffect(() => {
		if (drawsPlot(query())) {
			wantPlot();
		}
	});
	const drawn = createMemo(() => data()?.lines.map(({ id }) => id) ?? []);
	// The plot redraws only for what changed: a key toggle leaves the axes alone.
	const xAxis = createMemo(() => query().xAxis);
	const yScale = createMemo(() => query().yScale);
	const focused = createMemo(() => query().focus ?? null);
	const layout = createMemo(
		() => measuresLayout(query().measures.length, query().layout).layout,
	);
	const reserve = createMemo(
		() => ({
			lines: query().only?.length ?? 1,
			measures: Math.max(1, query().measures.length),
		}),
		undefined,
		{ equals: (a, b) => a.lines === b.lines && a.measures === b.measures },
	);
	const hidden = () => drawn().filter((id) => !lineVisible(query(), id));
	const settled = () => settleVisibility(query(), drawn());

	const [confirmation, setConfirmation] = createSignal<{
		shown: Confirmation;
		undo: () => Promise<void>;
	}>();
	const [filled, setFilled] = createSignal<string>();
	const [shared, setShared] = createSignal(false);
	const [working, setWorking] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	/** The reader changed the query. */
	const edit = (next: ExploreQuery, replace = false) => {
		setConfirmation(undefined);
		setFilled(undefined);
		setShared(false);
		setFailure(undefined);
		go(settleVisibility(next, drawn()), replace);
	};

	const plotResult = useQueryResult(() => ({
		...plotQuery(api, slug(), query().plot ?? ""),
		enabled: query().plot !== undefined,
	}));
	const pinned = () =>
		query().plot === undefined ? undefined : plotResult().data;
	// A bare link to a pin opens its query; a pin gone since is an unsaved plot.
	createEffect(() => {
		const plot = pinned();
		if (plot && !drawable(query())) {
			go(queryFromPlot(plot), true);
		}
		const { error } = plotResult();
		if (
			query().plot !== undefined &&
			error instanceof ApiError &&
			error.kind === "not_found"
		) {
			const { plot: _plot, ...rest } = query();
			go(rest, true);
		}
	});
	const dirty = () => {
		const plot = pinned();
		return plot !== undefined && unsaved(query(), plot, drawn());
	};
	const blank = () =>
		query().plot === undefined && query().benchmarks.length === 0;

	const variantsOf = async (benchmarks: string[]) =>
		new Map<string, readonly Parameters[]>(
			await Promise.all(
				benchmarks.map(
					async (benchmark) =>
						[
							benchmark,
							await client.query(variantsQuery(api, slug(), benchmark)),
						] as const,
				),
			),
		);
	const titleOf = async (current: ExploreQuery) => {
		const response = answer();
		if (!response) {
			return "";
		}
		const shown = shownAmong(current, drawn());
		const benchmarks = response.lines
			.filter(shown)
			.map(({ benchmark }) => response.benchmarks[benchmark]?.uuid)
			.filter((uuid): uuid is string => uuid !== undefined);
		return pinTitle(
			response,
			shown,
			await variantsOf([...new Set(benchmarks)]),
		);
	};
	/** Pin the query as a new plot at the top of Plots, and edit that pin. */
	const create = async (text: string) => {
		const before = query();
		setWorking(true);
		setFailure(undefined);
		try {
			const title = await titleOf(before);
			const plot = await pinPlot(
				api,
				slug(),
				newPlot(before, title, drawn(), Date.now()),
			);
			client.setQueryData(plotKey(slug(), plot.uuid), plot);
			client.invalidateQueries({ queryKey: ["console", "start", slug()] });
			go(queryFromPlot(plot), true);
			setConfirmation({
				shown: {
					text,
					title: plot.title ?? title,
					window: rollingName(plot.window),
				},
				undo: async () => {
					await unpinPlot(api, slug(), plot.uuid);
					client.removeQueries({ queryKey: plotKey(slug(), plot.uuid) });
					client.invalidateQueries({ queryKey: ["console", "start", slug()] });
					go(before, true);
				},
			});
		} catch {
			setFailure("Bencher did not pin the plot. Try again.");
		} finally {
			setWorking(false);
		}
	};
	const undo = async () => {
		const pin = confirmation();
		if (!pin) {
			return;
		}
		setWorking(true);
		try {
			await pin.undo();
			setConfirmation(undefined);
		} catch {
			setFailure("Bencher did not remove the pin. Try again.");
		} finally {
			setWorking(false);
		}
	};
	/** Write the query over the pin; the link then holds the pin as the API keeps it. */
	const save = async () => {
		const plot = pinned();
		if (!plot) {
			return false;
		}
		setWorking(true);
		setFailure(undefined);
		try {
			const { data: saved } = await api.send<JsonPlot>(
				"PATCH",
				`${project()}/plots/${plot.uuid}`,
				plotPatch(query(), drawn(), Date.now()),
			);
			client.setQueryData(plotKey(slug(), saved.uuid), saved);
			client.invalidateQueries({ queryKey: ["console", "start", slug()] });
			setConfirmation(undefined);
			go(queryFromPlot(saved), true);
			return true;
		} catch {
			setFailure("Bencher did not save the plot. Try again.");
			return false;
		} finally {
			setWorking(false);
		}
	};

	const share = () =>
		navigator.clipboard
			.writeText(window.location.href)
			.then(() => setShared(true))
			.catch(() => {});

	/** A benchmark picked first fills the branch, the testbed, and the measures from its latest report. */
	const first = async (option: Option) => {
		const before = query();
		let latest: JsonConsoleLatestReport;
		try {
			latest = await client.query(latestQuery(api, slug(), option.value));
		} catch {
			edit(withValue(before, "benchmarks", option.value));
			return;
		}
		const { branch, testbed, measures, version, start_time } = latest;
		const seed = (
			dimension: "branches" | "testbeds" | "measures",
			uuid: string,
			named: { name: string; units?: string },
		) =>
			client.setQueryData(nameQuery(api, slug(), dimension, uuid).queryKey, {
				name: named.name,
				...(named.units === undefined ? {} : { units: named.units }),
			});
		seed("branches", branch.uuid, branch);
		seed("testbeds", testbed.uuid, testbed);
		for (const measure of measures) {
			seed("measures", measure.uuid, measure);
		}
		edit({
			...withValue(before, "benchmarks", option.value),
			branches: [{ uuid: branch.uuid }],
			testbeds: [{ uuid: testbed.uuid }],
			measures: measures.slice(0, MAX_ENTRIES).map(({ uuid }) => uuid),
		});
		const hash = version.hash ? `${version.hash.slice(0, 7)} on ` : "";
		setFilled(
			`The branch, the testbed, and the measures came from ${option.label}'s latest report, ${hash}${formatDate(start_time)}. Change any of them.`,
		);
	};

	// Leaving a pin with unsaved changes for another page asks first; a link
	// press is held before the router takes it.
	const [leaving, setLeaving] = createSignal<string>();
	let discarded = false;
	const destination = (path: string) => {
		const place = parseNextPath(path);
		const tab = place ? tabOf(place.rest) : undefined;
		return { explore: place?.slug === slug() && tab === "explore", tab };
	};
	const hold = (event: MouseEvent) => {
		if (
			!dirty() ||
			event.defaultPrevented ||
			event.button !== 0 ||
			event.metaKey ||
			event.ctrlKey ||
			event.shiftKey ||
			event.altKey
		) {
			return;
		}
		const link =
			event.target instanceof Element ? event.target.closest("a[href]") : null;
		if (
			!(link instanceof HTMLAnchorElement) ||
			link.target === "_blank" ||
			link.origin !== window.location.origin ||
			destination(link.pathname).explore
		) {
			return;
		}
		event.preventDefault();
		setLeaving(`${link.pathname}${link.search}${link.hash}`);
	};
	document.addEventListener("click", hold, true);
	onCleanup(() => document.removeEventListener("click", hold, true));
	const unload = (event: BeforeUnloadEvent) => {
		if (dirty() && !discarded) {
			event.preventDefault();
		}
	};
	window.addEventListener("beforeunload", unload);
	onCleanup(() => window.removeEventListener("beforeunload", unload));
	const leave = () => {
		const to = leaving();
		setLeaving(undefined);
		if (to === undefined) {
			return;
		}
		discarded = true;
		if (to.startsWith(`${NEXT_PROJECTS}/`)) {
			navigate(to.slice(NEXT_PROJECTS.length));
		} else {
			window.location.assign(to);
		}
	};
	const leavingFor = () => {
		const to = leaving();
		const tab = to === undefined ? undefined : destination(to).tab;
		return TABS.find((entry) => entry.tab === tab)?.label ?? "another page";
	};

	const source = () => {
		if (blank()) {
			return "nothing chosen yet";
		}
		const alert = (location.state as { alert?: string } | undefined)?.alert;
		if (alert !== undefined) {
			return (
				<>
					from an alert on {alert}{" "}
					<a href={projectPath(slug(), "alerts")}>Alerts</a>
				</>
			);
		}
		const report = query().report;
		if (report === undefined) {
			return undefined;
		}
		const hash = answer()?.reports.find(({ uuid }) => uuid === report)?.hash;
		return (
			<>
				{hash ? `from report ${hash.slice(0, 7)} ` : "from a report "}
				<a href={reportPath(slug(), report)}>Open report</a>
			</>
		);
	};

	const [sheet, setSheet] = createSignal(false);

	// A query that cannot draw asks for its names at once; one that can waits for its answer.
	const editor = () => (
		<Editor
			query={query()}
			perf={answer()}
			last={perf().data}
			settled={
				!drawable(query()) || (!busy() && (perf().isSuccess || perf().isError))
			}
			blank={blank()}
			readOnly={props.readOnly}
			onQuery={(next) => edit(next)}
			onFirstBenchmark={first}
			onBenchmarks={wantPlot}
		/>
	);

	return (
		<main class="page ex">
			<section class="ex-qp" aria-label="Query">
				<Header
					pinned={
						query().plot === undefined
							? undefined
							: (pinned()?.title ?? "Pinned plot")
					}
					plotsHref={projectPath(slug(), "plots")}
					dirty={dirty()}
					source={source()}
					ready={bootstrap().data !== undefined}
					canPin={canPin()}
					hasLines={(data()?.lines.length ?? 0) > 0}
					working={working()}
					shared={shared()}
					confirmation={confirmation()?.shown}
					onShare={share}
					onPin={() => create("Pinned to the top of Plots as")}
					onUndo={undo}
					onDiscard={() => {
						const plot = pinned();
						if (plot) {
							edit(queryFromPlot(plot));
						}
					}}
					onSave={save}
					onSaveNew={() => create("Saved as a new pin at the top of Plots as")}
				/>
				<Show when={failure()}>
					<p class="ferror" role="alert">
						{failure()}
					</p>
				</Show>
				<Show when={narrow()}>
					<div class="chips ex-chips">
						<For each={DIMENSIONS}>
							{([dimension, label]) => (
								<Chip
									label={label}
									on={blank() && dimension === "benchmarks"}
									aria-haspopup="dialog"
									onClick={() => setSheet(true)}
								>
									<span class="ex-ellip">
										{chipSummary(query(), dimension, answer(), blank())}
									</span>
								</Chip>
							)}
						</For>
					</div>
					<Sheet open={sheet()} onClose={() => setSheet(false)} title="Query">
						{editor()}
					</Sheet>
				</Show>
				<Controls
					query={query()}
					total={answer()?.total}
					now={Date.now}
					onQuery={(next) => edit(next)}
				/>
				<Show when={!narrow()}>{editor()}</Show>
				<Show when={blank()}>
					<p class="ex-note">
						Add a benchmark to start. Its latest report fills the branch, the
						testbed, and the measures; change any of them after.
					</p>
				</Show>
				<Show when={filled()}>
					<p class="ex-note">{filled()}</p>
				</Show>
			</section>
			<Show when={drawable(query())}>
				<section class="ex-plot" aria-label="Plot" aria-busy={busy()}>
					<Show when={refused()}>
						{(text) => (
							<div class="ex-failed">
								<Banner status="error" role="alert" class="load-error">
									<span class="grow">{text()}</span>
									<Button
										size="lg"
										disabled={perf().isFetching}
										onClick={() => void perf().refetch()}
									>
										Retry
									</Button>
								</Banner>
							</div>
						)}
					</Show>
					<Show
						when={(answer() !== undefined || !refused()) && PlotView()}
						fallback={
							<PlotFrame
								reserve={reserve()}
								layout={layout()}
								loading={!refused()}
							/>
						}
					>
						{(View) => {
							const Loaded = View();
							return (
								<Loaded
									data={data()}
									hidden={hidden()}
									focused={focused()}
									onHiddenChange={(hide) => edit({ ...settled(), hide }, true)}
									onFocusChange={(focus) => {
										const { focus: _focus, ...rest } = settled();
										edit(focus === null ? rest : { ...rest, focus }, true);
									}}
									xAxis={xAxis()}
									scale={yScale()}
									layout={layout()}
									reportHref={(uuid) => reportPath(slug(), uuid)}
									reserve={reserve()}
								/>
							);
						}}
					</Show>
				</section>
			</Show>
			<Show when={blank() && !props.readOnly}>
				<Start />
			</Show>
			<Show when={leaving()}>
				<Leave
					title={pinned()?.title ?? "This pinned plot"}
					destination={leavingFor()}
					saving={working()}
					onStay={() => setLeaving(undefined)}
					onLeave={leave}
					onSave={async () => {
						if (await save()) {
							leave();
						}
					}}
				/>
			</Show>
		</main>
	);
};

export default Explore;

type PlotView = typeof import("../plot/Plot").default;
let plotView: PlotView | undefined;
let plotLoading: Promise<PlotView> | undefined;
/** The plot's code, with its charting library, which blank Explore never loads. */
const loadPlot = () => {
	plotLoading ??= import("../plot/Plot").then(
		({ default: view }) => {
			plotView = view;
			return view;
		},
		(error: unknown) => {
			plotLoading = undefined;
			throw error;
		},
	);
	return plotLoading;
};
