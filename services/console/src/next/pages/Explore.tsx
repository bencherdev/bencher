import { useLocation } from "@solidjs/router";
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
import type { JsonConsolePerf, JsonPlot } from "../../types/bencher";
import { ApiError } from "../api";
import ContentSkeleton from "../ContentSkeleton";
import Controls from "../explore/Controls";
import { plotData } from "../explore/data";
import { BOXES, clearable } from "../explore/edit";
import Editor from "../explore/Editor";
import Header, { type Confirmation } from "../explore/Header";
import { measuresLayout } from "../explore/layout";
import Leave from "../explore/Leave";
import { useAlertChange } from "../alerts/change";
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
	parseNextPath,
	alertPath,
	projectPath,
	reportPath,
	tabOf,
} from "../paths";
import IterationSelect from "../plot/IterationSelect";
import { loadPlot, loadedPlot } from "../plot/lazy";
import { useNarrow } from "../plot/narrow";
import EditPin from "../plots/EditPin";
import { PLOTS_LIMIT, countPins } from "../plots/pins";
import { useProject, useProjectNavigate } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import { exploreSearchFromClassic } from "../query/classic";
import { lineVisible, settleVisibility } from "../query/line";
import {
	type ExploreQuery,
	type Parameters,
	blankQuery,
	decodeQuery,
	encodeQuery,
} from "../query/query";

const DIMENSIONS = BOXES.map(
	(box) => [box, box === "sets" ? "parameters" : box] as const,
);

/** Explore: the editor for an unsaved plot, and for a pinned one; the query is the URL. */
const Explore = (props: { readOnly?: boolean | undefined }) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const location = useLocation();
	const navigate = useProjectNavigate();
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
	const alerts = useAlertChange().readout(
		() => !props.readOnly && bootstrap().data?.permissions.edit === true,
	);
	const full = () => (bootstrap().data?.plots ?? 0) >= PLOTS_LIMIT;
	// Plots and the start lists show pins, so a change here is theirs to read again.
	const pinsChanged = () => {
		client.invalidateQueries({ queryKey: ["console", "start", slug()] });
		client.invalidateQueries({ queryKey: ["console", "plots", slug()] });
	};

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
			? "The API refused this query"
			: "The plot did not load";
	};
	const [PlotView, setPlotView] = createSignal(loadedPlot());
	const [plotMissing, setPlotMissing] = createSignal(false);
	const wantPlot = (retry = false) => {
		if (!PlotView()) {
			setPlotMissing(false);
			loadPlot()
				.then((view) => setPlotView(() => view))
				.catch(() => {
					// A failed module, the chunk or one it imports, stays failed for the page's life: only reading the page again loads it.
					if (retry) {
						window.location.reload();
					} else {
						setPlotMissing(true);
					}
				});
		}
	};
	const plotFailure = () =>
		refused() ?? (plotMissing() ? "The plot did not load" : undefined);
	createEffect(() => {
		if (drawsPlot(query())) {
			wantPlot();
		}
	});
	const drawn = createMemo(() => data()?.lines.map(({ id }) => id) ?? []);
	// Iterations count from 0, so a report ran one more than its highest.
	const iterations = createMemo(() =>
		(data()?.iteration ?? []).reduce(
			(most, each) => Math.max(most, each + 1),
			1,
		),
	);
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
	const [shared, setShared] = createSignal(false);
	const [working, setWorking] = createSignal(false);
	const [failure, setFailure] = createSignal<string>();
	/** The reader changed the query. */
	const edit = (next: ExploreQuery, replace = false) => {
		setConfirmation(undefined);
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
	// A link to a pin not yet drawn here shows the page's skeleton until the
	// pin fills the query, so nothing moves as it arrives.
	const [drawnPin, setDrawnPin] = createSignal<string>();
	createEffect(() => {
		if (drawable(query())) {
			setDrawnPin(query().plot);
		}
	});
	const holding = () =>
		query().plot !== undefined &&
		query().plot !== drawnPin() &&
		!drawable(query()) &&
		!plotResult().isError;
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
			countPins(client, api, slug(), 1);
			pinsChanged();
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
					countPins(client, api, slug(), -1);
					pinsChanged();
					go(before, true);
				},
			});
		} catch {
			setFailure("Bencher did not pin the plot");
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
			setFailure("Bencher did not remove the pin");
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
			pinsChanged();
			setConfirmation(undefined);
			go(queryFromPlot(saved), true);
			return true;
		} catch {
			setFailure("Bencher did not save the plot");
			return false;
		} finally {
			setWorking(false);
		}
	};

	const [editing, setEditing] = createSignal<JsonPlot>();
	/** The pin dialog saved the pin just made: the link and the confirmation follow it. */
	const edited = (plot: JsonPlot) => {
		client.setQueryData(plotKey(slug(), plot.uuid), plot);
		pinsChanged();
		setEditing(undefined);
		setConfirmation(
			(pin) =>
				pin && {
					...pin,
					shown: {
						...pin.shown,
						title: plot.title ?? pin.shown.title,
						window: rollingName(plot.window),
					},
				},
		);
		go(queryFromPlot(plot), true);
	};

	const share = () =>
		navigator.clipboard
			.writeText(window.location.href)
			.then(() => setShared(true))
			.catch(() => {});

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

	const source = () => {
		if (blank()) {
			return "No lines";
		}
		const { alert, uuid } =
			(location.state as { alert?: string; uuid?: string } | undefined) ?? {};
		if (alert !== undefined) {
			return (
				<>
					from an{" "}
					<a
						href={
							uuid === undefined
								? projectPath(slug(), "alerts")
								: alertPath(slug(), uuid)
						}
					>
						alert
					</a>{" "}
					on {alert}
				</>
			);
		}
		const report = query().report;
		if (report === undefined) {
			return undefined;
		}
		const hash = answer()?.reports.find(({ uuid }) => uuid === report)?.hash;
		const link = (
			<a href={reportPath(slug(), report)}>
				{hash ? hash.slice(0, 7) : "report"}
			</a>
		);
		return hash ? <>from report {link}</> : <>from a {link}</>;
	};

	const [sheet, setSheet] = createSignal(false);

	// Clear's place is held while it has nothing to clear, so its arrival moves nothing.
	const clear = () => (
		<Show when={!props.readOnly}>
			<button
				type="button"
				class="lnk rp-clear ex-clear"
				classList={{ "ex-idle": !clearable(query()) }}
				onClick={(event) => {
					// The focus moves first to the first box, so it never falls with Clear.
					event.currentTarget
						.closest(".ex-qp")
						?.querySelector<HTMLElement>(".ui-chip, .ex-add")
						?.focus();
					edit(blankQuery());
				}}
			>
				Clear
			</button>
		</Show>
	);

	// A query that cannot draw asks for its names at once; one that can waits for its answer.
	const editor = () => (
		<Editor
			query={query()}
			perf={answer()}
			last={perf().data}
			settled={
				!drawable(query()) || (!busy() && (perf().isSuccess || perf().isError))
			}
			readOnly={props.readOnly}
			onQuery={(next) => edit(next)}
			onBenchmarks={() => wantPlot()}
		/>
	);

	const page = () => (
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
					full={full()}
					hasLines={(data()?.lines.length ?? 0) > 0}
					working={working()}
					shared={shared()}
					confirmation={confirmation()?.shown}
					onShare={share}
					onPin={() => create("Pinned")}
					onUndo={undo}
					onEdit={() => setEditing(pinned())}
					onDiscard={() => {
						const plot = pinned();
						if (plot) {
							edit(queryFromPlot(plot));
						}
					}}
					onSave={save}
					onSaveNew={() => create("Saved as new")}
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
									aria-haspopup="dialog"
									onClick={() => setSheet(true)}
								>
									<span class="ex-ellip">
										{chipSummary(query(), dimension, answer())}
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
				>
					<Show when={drawable(query())}>
						<IterationSelect
							count={iterations()}
							value={query().iteration}
							onChange={(iteration) => {
								const { iteration: _iteration, ...rest } = query();
								edit(iteration === undefined ? rest : { ...rest, iteration });
							}}
						/>
					</Show>
					{clear()}
				</Controls>
				<Show when={!narrow()}>{editor()}</Show>
			</section>
			<Show when={drawable(query())}>
				<section class="ex-plot" aria-label="Plot" aria-busy={busy()}>
					<Show when={plotFailure()}>
						{(text) => (
							<div class="ex-failed">
								<Banner status="error" role="alert" class="load-error">
									<span class="grow">{text()}</span>
									<Button
										size="lg"
										disabled={refused() !== undefined && perf().isFetching}
										onClick={() => {
											if (plotMissing()) {
												wantPlot(true);
											} else {
												void perf().refetch();
											}
										}}
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
								loading={!plotFailure()}
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
									iteration={query().iteration}
									reportHref={(uuid) => reportPath(slug(), uuid)}
									reserve={reserve()}
									alerts={alerts}
								/>
							);
						}}
					</Show>
				</section>
			</Show>
			<Show when={blank() && !props.readOnly}>
				<Start
					onQuery={(next) => {
						// The start lists go with the pick, so the focus moves to the box it filled.
						document
							.querySelector<HTMLElement>(
								'.ex-qp [aria-label="Benchmarks"] .ex-add, .ex-chips > :nth-child(3)',
							)
							?.focus();
						edit(next);
					}}
				/>
			</Show>
			<EditPin
				plot={editing()}
				onClose={() => setEditing(undefined)}
				onSaved={edited}
			/>
			<Show when={leaving()}>
				<Leave
					title={pinned()?.title ?? "This pinned plot"}
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
	return (
		<Show when={!holding()} fallback={<ContentSkeleton />}>
			{page()}
		</Show>
	);
};

export default Explore;
