import {
	batch,
	createEffect,
	createMemo,
	createSignal,
	createUniqueId,
	For,
	Index,
	on,
	onCleanup,
	onMount,
	Show,
	untrack,
} from "solid-js";
import {
	type CursorEvent,
	createPanel,
	type Panel,
	type PanelAxis,
} from "./chart";
import { alignToAxis, extent, sameX } from "./columns";
import { formatTick } from "./format";
import { constantParameters, keyOrder, lineNames } from "./key";
import { axisFont, axisWidth } from "./labels";
import { useNarrow } from "./narrow";
import { PlotKey, PlotKeyReserve } from "./PlotKey";
import { readout, readoutLeft } from "./readout";
import { yRange, yScale, yTicks } from "./scale";
import { type Layout, layoutOf, lineStyles, measuresOf } from "./series";
import { Swatch } from "./Swatch";
import { returnSyncKey, takeSyncKey } from "./sync";
import { usePalette } from "./theme";
import type {
	PlotData,
	PlotLayout,
	PlotMeasure,
	PlotScale,
	PlotSize,
	PlotXAxis,
} from "./types";
import { type UnitScale, unitScale } from "./units";

export interface PlotProps {
	/** Undefined while the data loads: the plot holds its frame. */
	data: PlotData | undefined;
	/** The ids of hidden lines, which the query owns. */
	hidden: readonly string[];
	/** The focused line's id, which the query owns: null for none, undefined for the only line or else the first alerting one. */
	focused?: string | null | undefined;
	onHiddenChange: (hidden: string[]) => void;
	onFocusChange: (focused: string | null) => void;
	xAxis?: PlotXAxis;
	scale?: PlotScale;
	/** Two measures share a dual axis unless stacked; three or more always stack. */
	layout?: PlotLayout;
	size?: PlotSize;
	/** Muted text in the head, such as the window. */
	note?: string;
	label?: string;
	/** Links the readout to the report a point came from. */
	reportHref?: (uuid: string) => string;
	/** The frame to hold before the data arrives. */
	reserve?: { lines: number; measures: number };
}

interface Cursor {
	index: number;
	panel: number;
	x: number;
	pinned: boolean;
}

const KEY_MAX = { full: 12, row: 12, tile: 4 } as const;
const NARROW_KEY_MAX = 6;

const plural = (count: number) => `${count} ${count === 1 ? "line" : "lines"}`;

const measureTitle = (measure: PlotMeasure | undefined, units: UnitScale) => {
	if (!measure) {
		return "";
	}
	return units.symbol &&
		units.symbol.toLowerCase() !== measure.name.toLowerCase()
		? `${measure.name}, ${units.symbol}`
		: measure.name;
};

/** One plot for Explore, a pinned tile, the public plot, and a row expanded in place. */
const Plot = (props: PlotProps) => {
	const syncKey = takeSyncKey();
	onCleanup(() => returnSyncKey(syncKey));
	const description = createUniqueId();
	const size = () => props.size ?? "full";
	const xAxis = () => props.xAxis ?? "date";
	const narrow = useNarrow();
	const palette = usePalette();

	const data = createMemo(() => props.data && alignToAxis(props.data, xAxis()));
	const lines = () => data()?.lines ?? [];
	const measures = createMemo(() => measuresOf(lines()));
	const layout = createMemo<Layout>(() =>
		layoutOf(
			data() ? measures().length : (props.reserve?.measures ?? 1),
			props.layout ?? "dual",
		),
	);
	const styles = createMemo(() => lineStyles(lines(), layout()));
	const names = createMemo(() => lineNames(lines(), data()?.measures ?? []));
	const order = createMemo(() => keyOrder(lines()));
	const units = createMemo(() => {
		const current = data();
		if (!current) {
			return [];
		}
		return current.measures.map((measure, index) => {
			const values = current.lines
				.filter((line) => line.measure === index)
				.map(({ y }) => ({ y }));
			return unitScale(extent(values)?.[0] ?? 0, measure.units);
		});
	});

	const hidden = createMemo(() => {
		const ids = new Set(props.hidden);
		const set = new Set<number>();
		lines().forEach((line, index) => {
			if (ids.has(line.id)) {
				set.add(index);
			}
		});
		return set;
	});
	const visible = (line: number | null) =>
		line !== null && line >= 0 && line < lines().length && !hidden().has(line);
	const resolved = createMemo<number | null>(() => {
		const focused = props.focused;
		if (focused === null) {
			return null;
		}
		if (focused !== undefined) {
			const line = lines().findIndex(({ id }) => id === focused);
			return visible(line) ? line : null;
		}
		const shown = lines()
			.map((_, index) => index)
			.filter((index) => !hidden().has(index));
		if (shown.length === 1) {
			return shown[0] ?? null;
		}
		return shown.find((index) => lines()[index]?.alerting) ?? null;
	});
	const [preview, setPreview] = createSignal<number | null>(null);
	const [hover, setHover] = createSignal<number | null>(null);
	const focus = createMemo<number | null>(() => {
		const keyed = preview();
		if (visible(keyed)) {
			return keyed;
		}
		const hovered = hover();
		return visible(hovered) ? hovered : resolved();
	});

	const [cursor, setCursor] = createSignal<Cursor | null>(null);
	const panels = new Map<number, Panel>();
	// Each plot area's offset in the body, read when it lays out so a hover reads no layout.
	const offsets = new Map<number, { left: number; top: number }>();
	createEffect(
		on(data, () => {
			batch(() => {
				setCursor(null);
				setHover(null);
			});
		}),
	);

	const panelSpecs = createMemo(() => {
		const current = data();
		if (!current) {
			return [];
		}
		const drawn = styles();
		const groups =
			layout() === "stacked"
				? measures().map((measure) => [measure])
				: [measures()];
		const specs = groups.map((group) => {
			const panelLines = current.lines
				.map((_, index) => index)
				.filter((index) => group.includes(current.lines[index]?.measure ?? -1));
			const axes: PanelAxis[] = group.map((measure) => ({
				measure,
				units: units()[measure] ?? { factor: 1, symbol: "" },
				lines: panelLines.filter(
					(index) => current.lines[index]?.measure === measure,
				),
			}));
			return { lines: panelLines, axes };
		});
		// Reserved from every line, so hiding lines rarely widens an axis; stacked plots share one.
		const font = axisFont(untrack(palette).code);
		const widths = specs.map(({ axes }) =>
			axes.map((axis) =>
				reservedWidth(current, axis, props.scale ?? "auto", font),
			),
		);
		const reserved = [0, 1].map((axis) =>
			Math.max(0, ...widths.map((panel) => panel[axis] ?? 0)),
		);
		return specs.map((spec, index) => ({
			...spec,
			size: size(),
			narrow: narrow(),
			styles: drawn,
			reserved,
			showX: index === specs.length - 1,
			titles: spec.axes.map((axis) =>
				measureTitle(current.measures[axis.measure], axis.units),
			),
		}));
	});

	const panelEvents = (panel: number) => ({
		hover: (event: CursorEvent | null) =>
			batch(() => {
				setCursor(
					event && { index: event.index, panel, x: event.x, pinned: false },
				);
				setHover(event?.line ?? null);
			}),
		pin: (event: CursorEvent) =>
			batch(() => {
				setCursor({ index: event.index, panel, x: event.x, pinned: true });
				if (event.line !== null) {
					setHover(event.line);
				}
			}),
		close: () =>
			batch(() => {
				setCursor(null);
				setHover(null);
			}),
	});

	// Each axis grows to fit the labels any plot draws, and stacked plots relay out to share the widest.
	const needs = [new Map<number, number>(), new Map<number, number>()];
	const shared = [0, 0];
	const widthFor =
		(panel: number, reserved: readonly number[]) =>
		(axis: number, need: number) => {
			const drawn = needs[axis];
			drawn?.set(panel, need);
			const width = Math.max(reserved[axis] ?? 0, ...(drawn?.values() ?? []));
			if (width !== shared[axis]) {
				shared[axis] = width;
				queueMicrotask(() => {
					for (const [index, other] of panels) {
						if (index !== panel) {
							other.relayout();
						}
					}
				});
			}
			return width;
		};

	const closeReadout = () => {
		const current = cursor();
		if (current) {
			panels.get(current.panel)?.clearCursor();
		}
		batch(() => {
			setCursor(null);
			setHover(null);
		});
	};

	createEffect(() => {
		hidden();
		focus();
		palette();
		untrack(() => {
			for (const panel of panels.values()) {
				panel.update();
			}
		});
	});

	const model = createMemo(() => {
		const current = cursor();
		const aligned = data();
		if (!current || !aligned) {
			return null;
		}
		return readout(
			aligned,
			current.index,
			order().filter((line) => !hidden().has(line)),
			focus(),
			units(),
			xAxis(),
		);
	});
	const placement = createMemo(() => {
		const current = cursor();
		const panel = current && panels.get(current.panel);
		const offset = current && offsets.get(current.panel);
		if (!current || !panel || !offset) {
			return null;
		}
		const frame = panel.frame();
		const x = offset.left + frame.left + current.x;
		const top = offset.top + frame.top;
		const focused = focus();
		const aligned = data();
		let ring: { top: number } | null = null;
		if (focused !== null && aligned) {
			const [start, end] = sameX(aligned.x, current.index);
			for (let index = end - 1; index >= start; index--) {
				const y = panel.yOf(focused, index);
				if (y !== null) {
					ring = { top: top + y };
					break;
				}
			}
		}
		return { x, top: top + 4, ring };
	});
	// Measured by resize observers, which report after layout, so a hover never forces one.
	const [room, setRoom] = createSignal(0);
	const [readoutWidth, setReadoutWidth] = createSignal(0);
	let body: HTMLDivElement | undefined;
	onMount(() => {
		if (!body) {
			return;
		}
		const observer = new ResizeObserver(([entry]) =>
			setRoom(entry?.borderBoxSize[0]?.inlineSize ?? 0),
		);
		observer.observe(body);
		onCleanup(() => observer.disconnect());
	});

	const keyMax = () => (narrow() ? NARROW_KEY_MAX : KEY_MAX[size()]);
	const alerts = () =>
		lines().filter((line, index) => line.alerting && !hidden().has(index))
			.length;
	const boundary = () => {
		const line = focus();
		const current = line === null ? undefined : lines()[line];
		if (line === null || !current || (!current.lower && !current.upper)) {
			return null;
		}
		const text = `boundary: ${names()[line]?.text ?? ""}`;
		return {
			slot: styles()[line]?.slot ?? 1,
			text: current.model ? `${text} · ${current.model}` : text,
		};
	};
	const head = () => size() !== "tile";
	const reportLink = (report: number) => {
		const uuid = data()?.reports[report]?.uuid;
		return uuid === undefined ? undefined : props.reportHref?.(uuid);
	};
	const reservedPanels = () =>
		layout() === "stacked" ? (props.reserve?.measures ?? 1) : 1;

	return (
		<figure
			class="pl"
			data-size={size()}
			data-layout={layout()}
			aria-label={
				props.label ??
				plural(data() ? lines().length : (props.reserve?.lines ?? 0))
			}
		>
			<span id={description} class="pl-hint">
				Arrow keys step through the reports, Home and End go to the first and
				the last, and Escape closes the readout.
			</span>
			<Show when={head()}>
				<div class="pl-head">
					<Index each={panelSpecs().flatMap(({ titles }) => titles)}>
						{(title) => <span class="pl-pill">{title()}</span>}
					</Index>
					<Show when={boundary()}>
						{(current) => (
							<span class="pl-pill on">
								<Swatch slot={current().slot} shape="circle" stroke="dashed" />
								<span class="pl-ellipsis">{current().text}</span>
							</span>
						)}
					</Show>
					<Show when={props.note}>
						<span class="pl-note">{props.note}</span>
					</Show>
					<span class="pl-spacer" />
					<Show when={alerts() > 0}>
						<span class="pl-pill alert">
							<span class="pl-alertdot" aria-hidden="true" />
							{alerts()} {alerts() === 1 ? "alert" : "alerts"}
						</span>
					</Show>
				</div>
			</Show>
			<div class="pl-body" ref={body}>
				<Show
					when={data()}
					fallback={
						<For each={Array.from({ length: reservedPanels() })}>
							{() => (
								<div class="pl-panel">
									<div class="pl-axes" />
									<div class="pl-area">
										<div
											class="pl-skel"
											role="img"
											aria-label="Loading the plot"
										/>
									</div>
								</div>
							)}
						</For>
					}
				>
					{(current) => (
						<For each={panelSpecs()}>
							{(spec, index) => {
								let area: HTMLDivElement | undefined;
								onMount(() => {
									if (!area) {
										return;
									}
									const panel = createPanel(
										area,
										{
											data: current(),
											lines: spec.lines,
											styles: spec.styles,
											axes: spec.axes,
											scale: props.scale ?? "auto",
											xAxis: xAxis(),
											showX: spec.showX,
											size: spec.size,
											narrow: spec.narrow,
											axisWidth: widthFor(index(), spec.reserved),
											description,
											syncKey: layout() === "stacked" ? syncKey : null,
											measures: spec.axes
												.map(
													(axis) =>
														current().measures[axis.measure]?.name ?? "",
												)
												.join(" and "),
											width: area.clientWidth,
											height: area.clientHeight,
										},
										{
											hidden,
											focus,
											palette,
											pinned: () => cursor()?.pinned ?? false,
											index: () => cursor()?.index ?? null,
										},
										panelEvents(index()),
									);
									panels.set(index(), panel);
									const target = area;
									const place = () =>
										offsets.set(index(), {
											left: target.offsetLeft,
											top: target.offsetTop,
										});
									place();
									const observer = new ResizeObserver(() => {
										panel.setSize(target.clientWidth, target.clientHeight);
										place();
									});
									observer.observe(target);
									onCleanup(() => {
										observer.disconnect();
										panels.delete(index());
										offsets.delete(index());
										for (const drawn of needs) {
											drawn.delete(index());
										}
										panel.destroy();
									});
								});
								return (
									<div class="pl-panel">
										<div class="pl-axes">
											<span class="pl-axt">
												<Show when={spec.titles.length > 1}>
													<svg
														class="pl-axglyph"
														viewBox="0 0 18 8"
														aria-hidden="true"
													>
														<line x1="1" y1="4" x2="17" y2="4" />
													</svg>
												</Show>
												{spec.titles[0]}
											</span>
											<Show when={spec.titles[1]}>
												<span class="pl-axt">
													{spec.titles[1]}
													<svg
														class="pl-axglyph"
														data-stroke="dotted"
														viewBox="0 0 18 8"
														aria-hidden="true"
													>
														<line x1="1" y1="4" x2="17" y2="4" />
													</svg>
												</span>
											</Show>
										</div>
										<div class="pl-area" ref={area} />
									</div>
								);
							}}
						</For>
					)}
				</Show>
				<div
					class="pl-live"
					role="status"
					aria-live={cursor()?.pinned ? "polite" : "off"}
				>
					<Show when={model()}>
						{(current) => (
							<Readout
								readout={current()}
								placement={placement()}
								left={readoutLeft(placement()?.x ?? 0, readoutWidth(), room())}
								onWidth={setReadoutWidth}
								pinned={cursor()?.pinned ?? false}
								single={lines().length === 1}
								names={names().map(({ text }) => text)}
								styles={styles()}
								hash={data()?.reports[current().report]?.hash}
								href={reportLink(current().report)}
								onClose={closeReadout}
							/>
						)}
					</Show>
				</div>
			</div>
			<Show
				when={data()}
				fallback={
					<PlotKeyReserve
						lines={props.reserve?.lines ?? 0}
						max={keyMax()}
						compact={size() === "tile"}
					/>
				}
			>
				<PlotKey
					lines={lines()}
					names={names()}
					styles={styles()}
					order={order()}
					hidden={hidden()}
					focused={resolved()}
					constants={constantParameters(lines())}
					max={keyMax()}
					compact={size() === "tile"}
					hint={size() === "full" && !narrow()}
					onToggle={(line) => {
						const id = lines()[line]?.id;
						if (id === undefined) {
							return;
						}
						props.onHiddenChange(
							props.hidden.includes(id)
								? props.hidden.filter((hiddenId) => hiddenId !== id)
								: [...props.hidden, id],
						);
					}}
					onFocus={(line) => {
						const id = lines()[line]?.id;
						if (id === undefined) {
							return;
						}
						if (props.hidden.includes(id)) {
							props.onHiddenChange(
								props.hidden.filter((hiddenId) => hiddenId !== id),
							);
						}
						props.onFocusChange(resolved() === line ? null : id);
					}}
					onPreview={setPreview}
				/>
			</Show>
		</figure>
	);
};

const reservedWidth = (
	data: PlotData,
	axis: PanelAxis,
	mode: PlotScale,
	font: string,
) => {
	const range = extent(axis.lines.flatMap((line) => data.lines[line] ?? []));
	if (!range) {
		return axisWidth([], font);
	}
	const [min, max] = range;
	const kind = yScale(mode, min, max);
	const [lo, hi] = yRange(kind, min, max);
	const { factor } = axis.units;
	const { ticks, step } = yTicks(kind, lo / factor, hi / factor, 5);
	return axisWidth(
		ticks.map((tick) => formatTick(tick, step)),
		font,
	);
};

interface ReadoutProps {
	readout: NonNullable<ReturnType<typeof readout>>;
	placement: {
		x: number;
		top: number;
		ring: { top: number } | null;
	} | null;
	left: number;
	onWidth: (width: number) => void;
	pinned: boolean;
	single: boolean;
	names: readonly string[];
	styles: ReturnType<typeof lineStyles>;
	hash: string | undefined;
	href: string | undefined;
	onClose: () => void;
}

const Readout = (props: ReadoutProps) => {
	const when = () =>
		props.hash ? `${props.readout.when} · ${props.hash}` : props.readout.when;
	let box: HTMLDivElement | undefined;
	onMount(() => {
		if (!box) {
			return;
		}
		const observer = new ResizeObserver(([entry]) =>
			props.onWidth(entry?.borderBoxSize[0]?.inlineSize ?? 0),
		);
		observer.observe(box);
		onCleanup(() => observer.disconnect());
	});
	return (
		<>
			<Show when={props.placement?.ring}>
				{(ring) => (
					<span
						class="pl-ring"
						style={{
							left: `${props.placement?.x ?? 0}px`,
							top: `${ring().top}px`,
						}}
					/>
				)}
			</Show>
			<div
				ref={box}
				class="pl-readout"
				classList={{ pinned: props.pinned }}
				style={{
					left: `${props.left}px`,
					top: `${props.placement?.top ?? 0}px`,
				}}
			>
				<span class="pl-when">
					<Show when={props.href} fallback={when()}>
						{(href) => <a href={href()}>{when()}</a>}
					</Show>
				</span>
				<For each={props.readout.rows}>
					{(row) => {
						const style = () => props.styles[row.line];
						return (
							<div class="pl-rrow" classList={{ focused: row.focused }}>
								<Show when={!props.single && style()}>
									{(current) => (
										<>
											<Swatch
												slot={current().slot}
												shape={current().shape}
												stroke={current().axis === 1 ? "dotted" : "solid"}
											/>
											<span class="pl-rname">{props.names[row.line]}</span>
										</>
									)}
								</Show>
								<b class="pl-mono">{row.value}</b>
								<Show when={row.delta}>
									{(delta) => (
										<span
											class="pl-delta"
											data-tone={delta().tone ?? undefined}
										>
											{delta().tone
												? `${delta().text} ${delta().tone}`
												: delta().text}
										</span>
									)}
								</Show>
								<Show when={row.limit}>
									<span class="pl-limit">{row.limit}</span>
								</Show>
							</div>
						);
					}}
				</For>
				<Show when={props.readout.more > 0}>
					<span class="pl-more">
						{props.readout.more} more{" "}
						{props.readout.more === 1 ? "line" : "lines"}
					</span>
				</Show>
				<Show when={props.pinned}>
					<button
						type="button"
						class="pl-close"
						aria-label="Close the readout"
						onClick={() => props.onClose()}
					>
						<svg
							viewBox="0 0 24 24"
							width="14"
							height="14"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							aria-hidden="true"
						>
							<path d="M6 6l12 12M18 6L6 18" />
						</svg>
					</button>
				</Show>
			</div>
		</>
	);
};

export default Plot;
