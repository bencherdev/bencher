import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Segmented from "@bencherdev/ui/Segmented";
import Skeleton from "@bencherdev/ui/Skeleton";
import { useQueryClient } from "@tanstack/solid-query";
import {
	For,
	Match,
	Show,
	Switch,
	batch,
	createComputed,
	createMemo,
	createSignal,
	mapArray,
	on,
} from "solid-js";
import type {
	JsonConsolePlot,
	JsonConsolePlotTitle,
	JsonConsolePlots,
	JsonNewPlot,
	JsonPlot,
} from "../../types/bencher";
import CustomRange from "../explore/CustomRange";
import { plotSearch, queryFromPlot } from "../explore/pinned";
import { perfQuery, plotKey } from "../explore/queries";
import { PRESETS, rollingName } from "../explore/window";
import { projectPath } from "../paths";
import EditPin from "../plots/EditPin";
import { Down, Plus, Reorder, Up } from "../plots/glyphs";
import { type Move, moveIndex, placed } from "../plots/order";
import { PLOTS_LIMIT, countPins } from "../plots/pins";
import { type PageWindow, plotsQuery, screenTiles } from "../plots/queries";
import Tile, { listed } from "../plots/Tile";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";

const WINDOWS = [
	...PRESETS.map(({ value }) => ({ value, label: value })),
	{ value: "custom", label: "Custom" },
] as const;
type WindowChoice = (typeof WINDOWS)[number]["value"];

const MOVED: Record<Move, string> = {
	up: "up",
	down: "down",
	top: "to the top",
	bottom: "to the bottom",
};
const DAY = 24 * 60 * 60 * 1000;

const plural = (count: number) =>
	`${count} ${count === 1 ? "pinned plot" : "pinned plots"}`;

/** A change shown before the API confirms it. */
interface Pending {
	apply: (order: string[]) => string[];
}

const Plots = () => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	const permissions = () => bootstrap().data?.permissions;
	const plotsPath = () => `/v0/projects/${encodeURIComponent(slug())}/plots`;
	const plotsCache = () => ({ queryKey: ["console", "plots", slug()] });

	const [pageWindow, setPageWindow] = createSignal<PageWindow>();
	const perPage = screenTiles();
	// A page window asks again for every batch asked for so far.
	const [count, setCount] = createSignal(1);
	createComputed(on(slug, () => setCount(1), { defer: true }));
	const queries = createMemo(
		mapArray(
			() => Array.from({ length: count() }, (_, index) => index + 1),
			(page) =>
				useQueryResult(() => ({
					...plotsQuery(api, slug(), pageWindow(), { page, perPage }),
					// Tiles drawn stay, marked busy, until the page window's answer arrives.
					placeholderData: (
						previous: JsonConsolePlots | undefined,
						last: { queryKey: readonly unknown[] } | undefined,
					) => (last?.queryKey[2] === slug() ? previous : undefined),
				})),
		),
	);
	const batches = () => queries().map((query) => query());
	const first = () => batches()[0];
	const failedBatches = () => batches().filter((each) => each.isLoadingError);

	// The order as the API last confirmed it, which a refused change returns to.
	const [confirmed, setConfirmed] = createSignal<JsonConsolePlotTitle[]>();
	createComputed(
		on(
			() => first()?.data?.order,
			(order) => {
				if (order) {
					setConfirmed(order);
				}
			},
		),
	);
	const [pending, setPending] = createSignal<readonly Pending[]>([]);
	const shown = createMemo(() =>
		pending().reduce(
			(order, change) => change.apply(order),
			(confirmed() ?? []).map(({ uuid }) => uuid),
		),
	);
	const titles = createMemo(
		() => new Map((confirmed() ?? []).map(({ uuid, title }) => [uuid, title])),
	);
	const titleOf = (uuid: string) => titles().get(uuid) ?? "Untitled";
	const confirmOrder = (
		change: (order: string[]) => string[],
		added?: JsonConsolePlotTitle,
	) => {
		const reorder = (entries: readonly JsonConsolePlotTitle[]) => {
			const byUuid = new Map(entries.map((entry) => [entry.uuid, entry]));
			if (added) {
				byUuid.set(added.uuid, added);
			}
			return change(entries.map(({ uuid }) => uuid)).flatMap(
				(uuid) => byUuid.get(uuid) ?? [],
			);
		};
		setConfirmed((entries) => entries && reorder(entries));
		client.setQueriesData<JsonConsolePlots>(
			plotsCache(),
			(data) => data && { ...data, order: reorder(data.order) },
		);
	};
	const settle = (change: Pending, confirm?: (order: string[]) => string[]) =>
		batch(() => {
			if (confirm) {
				confirmOrder(confirm);
			}
			setPending((all) => all.filter((each) => each !== change));
		});

	// Tiles by pin, from every batch loaded and from a pin Undo pinned again.
	const [restored, setRestored] = createSignal(
		new Map<string, JsonConsolePlot>(),
	);
	const tiles = createMemo(() => {
		const all = new Map(restored());
		for (const each of batches()) {
			for (const tile of each.data?.plots ?? []) {
				all.set(tile.plot.uuid, tile);
			}
		}
		return all;
	});
	// A tile nearing the screen asks for its batch; one a move brought into a
	// batch already loaded asks for that batch again, once.
	const asked = new Set<string>();
	const want = (uuid: string) => {
		if (tiles().has(uuid)) {
			return;
		}
		const page = Math.floor(shown().indexOf(uuid) / perPage) + 1;
		if (page > count()) {
			setCount(page);
			return;
		}
		const loaded = batches()[page - 1];
		if (loaded?.data && !loaded.isFetching && !asked.has(uuid)) {
			asked.add(uuid);
			void loaded.refetch();
		}
	};
	// The pins of every batch that failed with nothing to draw.
	const unloaded = createMemo(() => {
		const order = confirmed() ?? [];
		return batches().flatMap((each, index) =>
			each.isLoadingError
				? order.slice(index * perPage, (index + 1) * perPage)
				: [],
		);
	});
	const unloadedUuids = createMemo(
		() => new Set(unloaded().map(({ uuid }) => uuid)),
	);
	// Explore's start lists and a pin opened there read what changed here.
	const changed = (removed?: string) => {
		void client.invalidateQueries({ queryKey: ["console", "start", slug()] });
		if (removed) {
			client.removeQueries({ queryKey: plotKey(slug(), removed) });
		}
	};
	const savedPlot = async (uuid: string) =>
		tiles().get(uuid)?.plot ??
		(await api.get<JsonPlot>(`${plotsPath()}/${encodeURIComponent(uuid)}`))
			.data;

	const [failure, setFailure] = createSignal<string>();
	const [status, setStatus] = createSignal("");
	/**
	 * Move a pin, then focus `target`, by default what had the focus: moving a
	 * tile's node takes the focus out of it.
	 */
	const move = async (
		uuid: string,
		how: Move,
		target: () => HTMLElement | undefined = focusedElement(),
	) => {
		const order = shown();
		const index = moveIndex(order, uuid, how);
		if (index === undefined) {
			return;
		}
		const apply = (current: string[]) => placed(current, uuid, index);
		const change = { apply };
		batch(() => {
			setPending((all) => [...all, change]);
			setFailure(undefined);
			setStatus(
				`Moved ${titleOf(uuid)} ${MOVED[how]}: now ${index + 1} of ${order.length}.`,
			);
		});
		target()?.focus();
		try {
			await api.send("PATCH", `${plotsPath()}/${encodeURIComponent(uuid)}`, {
				index,
			});
			settle(change, apply);
			changed();
		} catch {
			batch(() => {
				settle(change);
				setStatus("");
				setFailure("Bencher did not move the plot. Try again.");
			});
			if (document.activeElement === document.body) {
				target()?.focus();
			}
		}
	};

	const [unpinned, setUnpinned] = createSignal<{
		plot: JsonPlot;
		tile: JsonConsolePlot | undefined;
		at: number;
	}>();
	const [saved, setSaved] = createSignal<JsonPlot>();
	const unpin = async (uuid: string) => {
		const at = shown().indexOf(uuid);
		const change = {
			apply: (order: string[]) => order.filter((each) => each !== uuid),
		};
		batch(() => {
			setPending((all) => [...all, change]);
			setUnpinned(undefined);
			setSaved(undefined);
			setFailure(undefined);
		});
		try {
			const plot = await savedPlot(uuid);
			const tile = tiles().get(uuid);
			await api.send("DELETE", `${plotsPath()}/${encodeURIComponent(uuid)}`);
			batch(() => {
				settle(change, change.apply);
				setUnpinned({ plot, tile, at });
				countPins(client, api, slug(), -1);
			});
			changed(uuid);
		} catch {
			batch(() => {
				settle(change);
				setFailure("Bencher did not unpin the plot. Try again.");
			});
		}
	};
	const undo = async () => {
		const gone = unpinned();
		if (!gone) {
			return;
		}
		setUnpinned(undefined);
		try {
			const { data: plot } = await api.send<JsonPlot>(
				"POST",
				plotsPath(),
				repin(gone.plot, gone.at),
			);
			batch(() => {
				const tile = gone.tile;
				if (tile) {
					setRestored((all) => new Map(all).set(plot.uuid, { ...tile, plot }));
				}
				confirmOrder((order) => placed(order, plot.uuid, gone.at), {
					uuid: plot.uuid,
					...(plot.title === undefined ? {} : { title: plot.title }),
				});
				countPins(client, api, slug(), 1);
			});
			changed();
		} catch {
			batch(() => {
				setUnpinned(gone);
				setFailure("Bencher did not pin the plot again. Try again.");
			});
		}
	};

	const [editing, setEditing] = createSignal<JsonPlot>();
	const edit = async (uuid: string) => {
		setFailure(undefined);
		seed(tiles().get(uuid));
		try {
			setEditing(await savedPlot(uuid));
		} catch {
			setFailure("Bencher did not load the pin. Try again.");
		}
	};
	const onSaved = (plot: JsonPlot, order: string[]) => {
		batch(() => {
			confirmOrder(() => order, {
				uuid: plot.uuid,
				...(plot.title === undefined ? {} : { title: plot.title }),
			});
			client.setQueriesData<JsonConsolePlots>(
				plotsCache(),
				(data) =>
					data && {
						...data,
						plots: data.plots.map((tile) =>
							tile.plot.uuid === plot.uuid ? { ...tile, plot } : tile,
						),
					},
			);
			client.setQueryData(plotKey(slug(), plot.uuid), plot);
			setEditing(undefined);
			setUnpinned(undefined);
			setSaved(plot);
		});
		changed();
		// The window may have changed, so the lines drawn at it come again.
		void client.invalidateQueries(plotsCache());
	};

	const [reordering, setReordering] = createSignal(false);
	const range = () => {
		const current = pageWindow();
		return typeof current === "object" ? current : undefined;
	};
	const choice = (): WindowChoice | undefined =>
		range() ? "custom" : (pageWindow() as WindowChoice | undefined);
	const pickWindow = (next: WindowChoice) => {
		if (next !== "custom") {
			setPageWindow(next);
		} else if (!range()) {
			const now = Date.now();
			setPageWindow({ start: now - 28 * DAY, end: now });
		}
	};
	const openHref = (tile: JsonConsolePlot | undefined, uuid: string) => {
		const explore = projectPath(slug(), "explore");
		return tile
			? `${explore}${plotSearch(tile.plot)}`
			: `${explore}?plot=${encodeURIComponent(uuid)}`;
	};
	// Explore and the pin dialog draw a pin at once from what its tile holds.
	const seed = (tile: JsonConsolePlot | undefined) => {
		if (!tile) {
			return;
		}
		client.setQueryData(plotKey(slug(), tile.plot.uuid), tile.plot);
		if (pageWindow() === undefined) {
			client.setQueryData(
				perfQuery(api, slug(), queryFromPlot(tile.plot)).queryKey,
				tile.perf,
			);
		}
	};

	// The head draws once both answers are in, so the read-only line moves nothing.
	const ready = () =>
		bootstrap().data !== undefined && confirmed() !== undefined;
	const total = () => shown().length;
	const canEdit = () => permissions()?.edit === true;
	const canCreate = () => permissions()?.create === true;
	const NewPlot = () => (
		<a
			class="ui-button"
			data-variant="primary"
			data-size="md"
			href={projectPath(slug(), "explore")}
		>
			<Plus />
			New plot
		</a>
	);

	return (
		<main class="page plots">
			<div class="plots-head">
				<div class="ph-title">
					<div class="crumb-line">Plots</div>
					<Show
						when={ready()}
						fallback={<Skeleton size="text" class="title-skel" />}
					>
						<Heading level={1} size="xl">
							{plural(total())}
						</Heading>
						<Show when={!(canEdit() || canCreate())}>
							<p class="ro-note">
								<Icon name="read-only" />
								Read only. Ask a project Maintainer for access.
							</p>
						</Show>
					</Show>
				</div>
				<div class="btnrow">
					<Show when={ready()}>
						<Show when={canEdit() || canCreate()}>
							<Show
								when={!reordering()}
								fallback={
									<Button
										variant="primary"
										onClick={() => setReordering(false)}
									>
										Done
									</Button>
								}
							>
								<Show when={canEdit() && total() > 1}>
									<Button
										onClick={() =>
											batch(() => {
												setReordering(true);
												setStatus("");
												setUnpinned(undefined);
												setSaved(undefined);
											})
										}
									>
										<Reorder />
										Reorder
									</Button>
								</Show>
								<Show when={canCreate()}>
									<NewPlot />
								</Show>
							</Show>
						</Show>
					</Show>
				</div>
			</div>
			<Show when={unpinned()}>
				{(gone) => (
					<p class="plots-confirm" role="status">
						<Icon name="check" />
						<span class="grow">
							Unpinned <b>"{gone().plot.title ?? "Untitled"}"</b>.
						</span>
						<Show when={canCreate()}>
							<button type="button" class="lnk" onClick={() => void undo()}>
								Undo
							</button>
						</Show>
					</p>
				)}
			</Show>
			<Show when={saved()}>
				{(plot) => (
					<p class="plots-confirm" role="status">
						<Icon name="check" />
						<span class="grow">
							Saved <b>"{plot().title ?? "Untitled"}"</b>,{" "}
							{rollingName(plot().window)} rolling.
						</span>
					</p>
				)}
			</Show>
			<Show when={failure()}>
				<p class="ferror" role="alert">
					{failure()}
				</p>
			</Show>
			<Show when={total() >= PLOTS_LIMIT}>
				<Banner status="warning" class="load-error">
					<span class="grow">
						<b>
							{total()} of {PLOTS_LIMIT} pinned.
						</b>{" "}
						A project keeps at most {PLOTS_LIMIT} pinned plots: unpin one before
						pinning another. Every Pin button says so until then.
					</span>
				</Banner>
			</Show>
			<Switch>
				<Match when={ready() && total() === 0}>
					<section class="ui-card plots-empty" aria-labelledby="plots-empty">
						<h2 id="plots-empty">No pinned plots yet</h2>
						<p>
							Pin a plot from Explore, a report, or an alert. One click puts it
							at the top of this page, at the window it was viewed at.
						</p>
						<Show when={canCreate()}>
							<NewPlot />
						</Show>
					</section>
				</Match>
				<Match when={ready() && reordering()}>
					<section class="ui-card plots-reorder" aria-labelledby="plots-reo">
						<div class="plots-reo-head">
							<h2 id="plots-reo" class="seclabel">
								Reorder
							</h2>
							<span class="muted sm">Each move saves at once.</span>
						</div>
						<ol class="plots-reo">
							<For each={shown()}>
								{(uuid, at) => {
									const title = createMemo(() => titleOf(uuid));
									const meta = () => {
										const perf = tiles().get(uuid)?.perf;
										if (!perf) {
											return "";
										}
										const lines = perf.lines.length;
										return [
											`${lines} ${lines === 1 ? "line" : "lines"}`,
											listed(perf.branches.map(({ name }) => name)),
											listed(perf.testbeds.map(({ name }) => name)),
										].join(" · ");
									};
									let up: HTMLButtonElement | undefined;
									let down: HTMLButtonElement | undefined;
									// At an end its own button is disabled, so the focus takes the other.
									const shift = (how: "up" | "down") => {
										const [own, other] = how === "up" ? [up, down] : [down, up];
										void move(uuid, how, () => (own?.disabled ? other : own));
									};
									return (
										<li>
											<span class="plots-num muted" aria-hidden="true">
												{at() + 1}
											</span>
											<span class="plots-reo-name">
												<b>{title()}</b>
												<span class="muted sm">{meta()}</span>
											</span>
											<Button
												ref={up}
												aria-label={`Move ${title()} up`}
												disabled={at() === 0}
												onClick={() => shift("up")}
											>
												<Up />
											</Button>
											<Button
												ref={down}
												aria-label={`Move ${title()} down`}
												disabled={at() === total() - 1}
												onClick={() => shift("down")}
											>
												<Down />
											</Button>
										</li>
									);
								}}
							</For>
						</ol>
						<p class="muted sm plots-status" role="status">
							{status()}
						</p>
					</section>
				</Match>
				<Match when={ready()}>
					<div class="toolbar plots-window">
						<span class="eyebrow" aria-hidden="true">
							Window
						</span>
						<Segmented
							name="plots-window"
							aria-label="Window for every plot on this page"
							options={WINDOWS}
							value={choice()}
							onChange={pickWindow}
						/>
						<Show when={range()}>
							{(dates) => (
								<CustomRange
									window={dates()}
									now={Date.now()}
									onWindow={(next) => {
										if ("start" in next) {
											setPageWindow({
												start: next.start,
												end: next.end ?? Date.now(),
											});
										}
									}}
								/>
							)}
						</Show>
						<span class="muted sm">
							{pageWindow() === undefined
								? "Each plot shows its own saved window."
								: `Showing ${range() ? "the chosen dates" : pageWindow()} on every plot, for this view only.`}
						</span>
						<Show when={pageWindow() !== undefined}>
							<button
								type="button"
								class="lnk"
								onClick={() => setPageWindow(undefined)}
							>
								Each plot keeps its own
							</button>
						</Show>
					</div>
					<div
						class="plots-grid"
						aria-busy={first()?.isPlaceholderData === true}
					>
						<For each={shown()}>
							{(uuid, at) => {
								// A confirmed order or batch redraws only the tiles it changed.
								const tile = createMemo(() => tiles().get(uuid));
								const title = createMemo(() => titleOf(uuid));
								const failed = createMemo(() => unloadedUuids().has(uuid));
								const window = () => tile()?.plot.window;
								return (
									<Tile
										title={title()}
										tile={tile()}
										failed={failed()}
										windowText={
											window() === undefined
												? ""
												: `saved window ${rollingName(window() ?? 0)}`
										}
										openHref={openHref(tile(), uuid)}
										onOpen={() => seed(tile())}
										onNear={() => want(uuid)}
										menu={
											canEdit()
												? {
														first: at() === 0,
														last: at() === total() - 1,
														canUnpin: permissions()?.delete === true,
														onRename: () => void edit(uuid),
														onMove: (how: Move) => void move(uuid, how),
														onUnpin: () => void unpin(uuid),
													}
												: undefined
										}
									/>
								);
							}}
						</For>
					</div>
					<Show when={unloaded().length > 0}>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">
								{listed(unloaded().map(({ title }) => title ?? "Untitled"))} did
								not load: the Bencher API did not answer.
							</span>
							<Button
								size="sm"
								onClick={() => {
									for (const each of failedBatches()) {
										void each.refetch();
									}
								}}
							>
								Retry
							</Button>
						</Banner>
					</Show>
					<Show when={status()}>
						<p class="sr-only" role="status">
							{status()}
						</p>
					</Show>
				</Match>
				<Match when={first()?.isLoadingError}>
					<Banner status="error" role="alert" class="load-error">
						<span class="grow">
							Plots did not load: the Bencher API did not answer.
						</span>
						<Button size="sm" onClick={() => void first()?.refetch()}>
							Retry
						</Button>
					</Banner>
				</Match>
				<Match when={true}>
					<div class="toolbar plots-window" />
					<div class="plots-grid" aria-busy="true">
						<span class="sr-only">Loading the pinned plots</span>
						<For each={[0, 1]}>{() => <div class="plots-tile" />}</For>
					</div>
				</Match>
			</Switch>
			<EditPin
				plot={editing()}
				order={confirmed()}
				rename
				onClose={() => setEditing(undefined)}
				onSaved={onSaved}
			/>
		</main>
	);
};

export default Plots;

/** A pin as it was saved, at `index`, to pin it again. */
const repin = (plot: JsonPlot, index: number): JsonNewPlot => {
	const {
		uuid: _uuid,
		project: _project,
		created: _created,
		modified: _modified,
		...saved
	} = plot;
	return { ...saved, index };
};

const focusedElement = () => {
	const element = document.activeElement;
	return () => (element instanceof HTMLElement ? element : undefined);
};
