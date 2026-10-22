import Menu, { MenuItem } from "@bencherdev/ui/Menu";
import {
	Show,
	createMemo,
	createSignal,
	createUniqueId,
	onCleanup,
	onMount,
} from "solid-js";
import type { JsonConsolePlot } from "../../types/bencher";
import { plotData } from "../explore/data";
import { queryFromPlot } from "../explore/pinned";
import { measuresLayout } from "../explore/layout";
import { rollingName } from "../explore/window";
import { loadPlot, loadedPlot } from "../plot/lazy";
import type { PlotAlerts } from "../plot/Plot";
import type { PlotData } from "../plot/types";
import type { Move } from "./order";

const DAY = 24 * 60 * 60 * 1000;

/** The frame a tile holds before its lines arrive: as many key entries as a tile shows. */
const RESERVE = { lines: 5, measures: 1 };

// One answer, one drawing: a tile whose data did not change keeps its plot.
const drawn = new WeakMap<JsonConsolePlot, PlotData>();
export const drawingOf = (tile: JsonConsolePlot) => {
	let data = drawn.get(tile);
	if (!data) {
		data = plotData(tile.perf);
		drawn.set(tile, data);
	}
	return data;
};

/** "a", "a and b", or "a and N more". */
export const listed = (names: readonly string[]): string => {
	const [first = "", second] = names;
	if (names.length > 2) {
		return `${first} and ${names.length - 1} more`;
	}
	return second === undefined ? first : `${first} and ${second}`;
};

const Tile = (props: {
	title: string;
	/** Undefined until its batch arrives. */
	tile: JsonConsolePlot | undefined;
	/** Its batch failed, so no lines are on the way. */
	failed?: boolean;
	/** Its own window, such as "4w", while it draws at it. */
	window?: string | undefined;
	openHref?: string;
	menu?:
		| {
				first: boolean;
				last: boolean;
				canUnpin: boolean;
				onRename: () => void;
				onMove: (move: Move) => void;
				onUnpin: () => void;
		  }
		| undefined;
	onNear?: () => void;
	onOpen?: () => void;
	class?: string;
	/** What a pinned readout on an alert offers. */
	alerts?: PlotAlerts | undefined;
}) => {
	const heading = createUniqueId();
	let section: HTMLElement | undefined;
	const [near, setNear] = createSignal(false);
	onMount(() => {
		if (!section) {
			return;
		}
		const observer = new IntersectionObserver(
			(entries) => {
				if (entries.some((entry) => entry.isIntersecting)) {
					setNear(true);
					props.onNear?.();
				}
			},
			{ rootMargin: "50% 0px" },
		);
		observer.observe(section);
		onCleanup(() => observer.disconnect());
	});

	// The plot's code loads beside the page's, so a tile waits on it once at most.
	const [View, setView] = createSignal(loadedPlot());
	onMount(() => {
		if (!View()) {
			loadPlot()
				.then((view) => setView(() => view))
				.catch(() => {});
		}
	});

	const data = createMemo(() =>
		props.tile ? drawingOf(props.tile) : undefined,
	);
	const query = createMemo(() => props.tile && queryFromPlot(props.tile.plot));
	const [hidden, setHidden] = createSignal<string[]>();
	const [focused, setFocused] = createSignal<string | null>();
	const meta = () => {
		const perf = props.tile?.perf;
		if (!perf) {
			return "";
		}
		return [perf.branches, perf.testbeds]
			.map((names) => listed(names.map(({ name }) => name)))
			.join(" · ");
	};
	const lines = () => data()?.lines.length;
	// An alert still active, as the page holds its alerts' statuses.
	const alerting = () =>
		data()?.lines.some((line) =>
			line.raised
				? line.raised.some(
						({ uuid, status }) =>
							(props.alerts?.status?.(uuid) ?? status) === "active",
					)
				: line.alerting,
		);
	// What the tile draws, as the answer says: its own window or the page's.
	const label = () => {
		const count = lines();
		const window = props.tile?.perf.window;
		if (count === undefined || !window) {
			return props.failed ? "The plot did not load" : "Loading the plot";
		}
		// A window ends at the reader's millisecond and starts on a whole second.
		const days = Math.round((window.end_time - window.start_time) / DAY);
		const drawn = rollingName((days * DAY) / 1000);
		return `${count === 1 ? "1 line" : `${count} lines`} over ${drawn}`;
	};

	const [open, setOpen] = createSignal(false);
	let more: HTMLButtonElement | undefined;
	const choose = (action: () => void) => {
		setOpen(false);
		action();
	};

	return (
		<section
			ref={section}
			class={props.class ? `plots-tile ${props.class}` : "plots-tile"}
			aria-labelledby={heading}
		>
			<div class="plots-thead">
				<h2 class="plots-ttl" id={heading}>
					{props.title}
				</h2>
				<span class="plots-meta">{meta()}</span>
				<span class="spacer" />
				<Show when={alerting()}>
					<span class="plots-alert" role="img" aria-label="has an alert" />
				</Show>
				<Show when={props.menu}>
					{(menu) => (
						<div class="plots-more">
							<button
								ref={more}
								type="button"
								class="plots-dots"
								aria-haspopup="menu"
								aria-expanded={open()}
								aria-label={`More for ${props.title}`}
								onClick={() => setOpen(!open())}
							>
								<svg
									viewBox="0 0 24 24"
									width="16"
									height="16"
									fill="currentColor"
									aria-hidden="true"
								>
									<circle cx="12" cy="5" r="1.8" />
									<circle cx="12" cy="12" r="1.8" />
									<circle cx="12" cy="19" r="1.8" />
								</svg>
							</button>
							<Show when={open()}>
								<Menu
									label={`More for ${props.title}`}
									anchor={more}
									align="end"
									onClose={() => setOpen(false)}
								>
									<MenuItem onClick={() => choose(menu().onRename)}>
										Rename
									</MenuItem>
									<MenuItem
										disabled={menu().first}
										onClick={() => choose(() => menu().onMove("up"))}
									>
										Move up
									</MenuItem>
									<MenuItem
										disabled={menu().last}
										onClick={() => choose(() => menu().onMove("down"))}
									>
										Move down
									</MenuItem>
									<MenuItem
										disabled={menu().first}
										onClick={() => choose(() => menu().onMove("top"))}
									>
										Move to top
									</MenuItem>
									<MenuItem
										disabled={menu().last}
										onClick={() => choose(() => menu().onMove("bottom"))}
									>
										Move to bottom
									</MenuItem>
									<Show when={menu().canUnpin}>
										<hr class="ui-menu-sep" />
										<MenuItem
											data-tone="danger"
											onClick={() => choose(menu().onUnpin)}
										>
											Unpin
										</MenuItem>
									</Show>
								</Menu>
							</Show>
						</div>
					)}
				</Show>
			</div>
			<Show when={View()} fallback={<div class="plots-frame" />}>
				{(view) => {
					const Loaded = view();
					return (
						<Loaded
							size="tile"
							data={near() ? data() : undefined}
							hidden={hidden() ?? query()?.hide ?? []}
							focused={
								focused() === undefined ? (query()?.focus ?? null) : focused()
							}
							onHiddenChange={setHidden}
							onFocusChange={setFocused}
							xAxis={query()?.xAxis ?? "date"}
							scale={query()?.yScale ?? "auto"}
							layout={
								measuresLayout(
									query()?.measures.length ?? 1,
									query()?.layout ?? "dual",
								).layout
							}
							reserve={RESERVE}
							label={label()}
							alerts={props.alerts}
							failed={props.failed ?? false}
						/>
					);
				}}
			</Show>
			<div class="plots-tfoot">
				<Show when={props.window}>
					<span class="lr-tag">{props.window}</span>
				</Show>
				<span class="spacer" />
				<Show when={props.openHref}>
					{(href) => (
						<a
							class="lnk"
							href={href()}
							aria-label={`Open ${props.title} in Explore`}
							onClick={() => props.onOpen?.()}
						>
							Open
						</a>
					)}
				</Show>
			</div>
		</section>
	);
};

export default Tile;
