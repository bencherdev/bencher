import {
	batch,
	createEffect,
	createMemo,
	createSignal,
	createUniqueId,
	For,
	on,
	onCleanup,
	onMount,
	Show,
	untrack,
} from "solid-js";
import Truncate from "@bencherdev/ui/Truncate";
import {
	type CursorEvent,
	createPanel,
	type Panel,
	type PanelAxis,
} from "./chart";
import { axisTicks, rangeTier } from "./axis";
import { alignToAxis, extent } from "./columns";
import { type AxisTier, axisTier, formatTicks } from "./format";
import { formatCount } from "./count";
import { onlyIteration, withThrough } from "./iterations";
import { keyOrder, type LineName, lineNames } from "./key";
import { axisFont, axisWidth } from "./labels";
import { useNarrow } from "./narrow";
import { NameParts, PlotKey, PlotKeyReserve } from "./PlotKey";
import { type Cursor, clicked, readout, readoutLeft } from "./readout";
import { type Layout, layoutOf, lineStyles, measuresOf } from "./series";
import { Swatch } from "./Swatch";
import { returnSyncKey, takeSyncKey } from "./sync";
import { usePalette } from "./theme";
import type {
	PlotAlert,
	PlotData,
	PlotLayout,
	PlotLine,
	PlotMeasure,
	PlotScale,
	PlotSize,
	PlotXAxis,
} from "./types";

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
	/** Muted text in the first axes row, such as the window. */
	note?: string | undefined;
	label?: string;
	/** Links the readout to the report a point came from. */
	reportHref?: (uuid: string) => string;
	/** The frame to hold before the data arrives. */
	reserve?: { lines: number; measures: number };
	/** No data is on its way: the frame says so instead of that it is loading. */
	failed?: boolean;
	/** The alerts behind the markers, which a pinned readout on one opens and changes. */
	alerts?: PlotAlerts | undefined;
	/** A point the readout pins as the plot first draws, an index into `x` on a date axis. */
	pin?: number | undefined;
	/** Only this iteration of every report, counting from 1; every iteration when undefined. */
	iteration?: number | undefined;
}

export interface PlotAlerts {
	/** The status the page holds for an alert, when newer than the data's. */
	status?: (uuid: string) => PlotAlert["status"] | undefined;
	/** The alert's own page; none on that page. */
	href?: (uuid: string) => string;
	/** Changes an alert's status; none for an alert the reader cannot change here. */
	change?: (
		uuid: string,
	) => ((status: "active" | "dismissed") => void) | undefined;
	/** The last change of the alert here did not go through. */
	refused?: (uuid: string) => boolean;
}

const KEY_MAX = { full: 12, row: 12, tile: 4 } as const;
const NARROW_KEY_MAX = 6;

const plural = (count: number) =>
	`${formatCount(count)} ${count === 1 ? "line" : "lines"}`;

/** The measure's name, and its unit's symbol where that says more. */
const measureTitle = (
	measure: PlotMeasure | undefined,
	tier: AxisTier,
): string[] => {
	if (!measure) {
		return [];
	}
	return tier.symbol && tier.symbol.toLowerCase() !== measure.name.toLowerCase()
		? [measure.name, `, ${tier.symbol}`]
		: [measure.name];
};

/** An axis title whose name and unit share its room, so neither is cut first. */
const AxisTitle = (props: { parts: string[] }) => (
	<span class="ui-parts pl-axparts">
		<For each={props.parts}>{(part) => <Truncate text={part} />}</For>
	</span>
);

/** One plot for Explore, a pinned tile, the public plot, and a row expanded in place. */
const Plot = (props: PlotProps) => {
	const syncKey = takeSyncKey();
	onCleanup(() => returnSyncKey(syncKey));
	const description = createUniqueId();
	const size = () => props.size ?? "full";
	const xAxis = () => props.xAxis ?? "date";
	const narrow = useNarrow();
	const palette = usePalette();

	// An iteration no report ran, or a choice where every report ran once, draws every iteration.
	const picked = createMemo(() => {
		const { data: given, iteration } = props;
		const only =
			given && iteration !== undefined
				? onlyIteration(given, iteration)
				: undefined;
		return only?.x.length
			? { data: only, iteration }
			: { data: given, iteration: undefined };
	});
	const data = createMemo(() => {
		const given = picked().data;
		return given && withThrough(alignToAxis(given, xAxis()));
	});
	const lines = () => data()?.lines ?? [];
	const measures = createMemo(() => measuresOf(lines()));
	const layout = createMemo<Layout>(() =>
		layoutOf(
			data() ? measures().length : (props.reserve?.measures ?? 1),
			props.layout ?? "dual",
		),
	);
	const styles = createMemo(() => lineStyles(lines(), layout()));
	// Once per line set, so a toggle or a hover names nothing anew.
	const named = createMemo(() => lineNames(lines(), data()?.measures ?? []));
	const names = () => named().names;
	const order = createMemo(() => keyOrder(lines()));

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
	// Each line's extent once per data, so a toggle only combines the shown ones.
	const extents = createMemo(() => lines().map((line) => extent([line])));
	const tiers = createMemo(
		() => {
			const current = data();
			return current
				? axisTiers(current, extents(), hidden(), props.scale ?? "auto")
				: [];
		},
		[],
		{ equals: sameTiers },
	);
	const tierOf = (measure: number) => tiers()[measure] ?? axisTier(0, "");
	const titleOf = (axis: PanelAxis | undefined) =>
		axis
			? measureTitle(data()?.measures[axis.measure], tierOf(axis.measure))
			: [];
	const statusOf = (alert: PlotAlert) =>
		props.alerts?.status?.(alert.uuid) ?? alert.status;
	/** A line with an alert still active, as the page holds its alerts' statuses. */
	const alerting = (line: PlotLine | undefined) =>
		line?.raised
			? line.raised.some((alert) => statusOf(alert) === "active")
			: line?.alerting === true;
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
	// Whether the keys put the cursor where it is, so the readout does not offer a click.
	const [byKeys, setByKeys] = createSignal(false);
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
				lines: panelLines.filter(
					(index) => current.lines[index]?.measure === measure,
				),
			}));
			return { lines: panelLines, axes };
		});
		// Reserved from every line and from each alone, so hiding lines rarely widens an axis; stacked plots share one.
		const font = axisFont(untrack(palette).code);
		const widths = specs.map(({ axes }) =>
			axes.map((axis) =>
				Math.max(
					...[axis.lines, ...axis.lines.map((line) => [line])].map((lines) =>
						reservedWidth(
							current,
							lines,
							current.measures[axis.measure]?.units ?? "",
							props.scale ?? "auto",
							font,
						),
					),
				),
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
		}));
	});

	const panelEvents = (panel: number) => ({
		hover: (event: CursorEvent | null) =>
			batch(() => {
				setCursor(event && { ...event, panel, pinned: false });
				setHover(event?.line ?? null);
				setByKeys(false);
			}),
		pin: (event: CursorEvent) =>
			batch(() => {
				setCursor({ ...event, panel, pinned: true });
				if (event.line !== null) {
					setHover(event.line);
				}
				setByKeys(true);
			}),
		click: (event: CursorEvent, touch: boolean) =>
			batch(() => {
				const next = clicked(
					cursor(),
					{ ...event, panel },
					data()?.x ?? [],
					touch,
					data()?.iteration,
				);
				setCursor(next);
				if (!next) {
					panels.get(panel)?.clearCursor();
					setHover(null);
				} else if (event.line !== null || !next.pinned) {
					setHover(event.line);
				}
				setByKeys(false);
			}),
		toggle: () =>
			batch(() => {
				const current = cursor();
				if (current) {
					setCursor({ ...current, pinned: !current.pinned });
				}
				setByKeys(true);
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
		for (const line of lines()) {
			line.raised?.forEach(statusOf);
		}
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
			xAxis(),
			undefined,
			{ line: current.line, chosen: picked().iteration },
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
		// The focused line leads the readout, which knows the point it reads.
		const row = model()?.rows[0];
		const y = row?.focused ? panel.yOf(row.line, row.point) : null;
		return { x, top: top + 4, ring: y === null ? null : { top: top + y } };
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

	const keyMax = () =>
		narrow() ? Math.min(NARROW_KEY_MAX, KEY_MAX[size()]) : KEY_MAX[size()];
	const alerts = () =>
		lines().filter((line, index) => alerting(line) && !hidden().has(index))
			.length;
	const boundary = () => {
		const line = focus();
		const current = line === null ? undefined : lines()[line];
		if (line === null || !current || (!current.lower && !current.upper)) {
			return null;
		}
		return {
			slot: styles()[line]?.slot ?? 1,
			name: names()[line] as LineName,
			model: current.model,
		};
	};
	/** The first panel's axes row heads the plot, which a tile does without. */
	const lead = (panel: number) => panel === 0 && size() !== "tile";
	const reportLink = (report: number) => {
		const uuid = data()?.reports[report]?.uuid;
		return uuid === undefined ? undefined : props.reportHref?.(uuid);
	};
	/** The alert at the focused line's pinned point, as its readout offers it. */
	const pinnedAlert = createMemo(() => {
		const row = cursor()?.pinned ? model()?.rows[0] : undefined;
		const alert = row?.focused ? row.alert : undefined;
		const line = row && data()?.lines[row.line];
		if (!(alert && line && props.alerts)) {
			return undefined;
		}
		const status = statusOf(alert);
		const change = props.alerts.change?.(alert.uuid);
		const href = props.alerts.href?.(alert.uuid);
		const action: "Dismiss" | "Reactivate" | undefined =
			change && status !== "silenced"
				? status === "active"
					? "Dismiss"
					: "Reactivate"
				: undefined;
		return href
			? {
					name: names()[row.line]?.text ?? "",
					href,
					action,
					refused: props.alerts.refused?.(alert.uuid) === true,
					run: () => {
						change?.(status === "active" ? "dismissed" : "active");
					},
				}
			: undefined;
	});
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
				the last, Enter pins or unpins, and Escape closes the readout.
			</span>
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
											aria-label={
												props.failed
													? "The plot did not load"
													: "Loading the plot"
											}
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
											status: statusOf,
											tier: tierOf,
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
									// Pinned once the panel has its size, so the readout sits at the point.
									let pin = index() === 0 ? props.pin : undefined;
									const observer = new ResizeObserver(() => {
										panel.setSize(target.clientWidth, target.clientHeight);
										place();
										if (pin !== undefined) {
											// On the focused line, so the readout emphasizes this point rather than the iteration the line runs through.
											panel.pinAt(pin, focus());
											pin = undefined;
										}
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
											<span class="pl-pill pl-axt">
												<Show when={spec.axes.length > 1}>
													<svg
														class="pl-axglyph"
														viewBox="0 0 12 8"
														aria-hidden="true"
													>
														<line x1="1" y1="4" x2="11" y2="4" />
													</svg>
												</Show>
												<AxisTitle parts={titleOf(spec.axes[0])} />
											</span>
											<Show when={lead(index()) && boundary()}>
												{(current) => (
													<span class="pl-pill on">
														<Swatch
															slot={current().slot}
															shape="circle"
															stroke="dashed"
														/>
														<span>boundary:</span>{" "}
														<span class="pl-pname ui-parts">
															<NameParts name={current().name} cut />
														</span>
														<Show when={current().model}>
															{(model) => (
																<span class="pl-model"> · {model()}</span>
															)}
														</Show>
													</span>
												)}
											</Show>
											<Show when={lead(index()) && props.note}>
												<span class="pl-note">{props.note}</span>
											</Show>
											<Show when={lead(index()) && alerts() > 0}>
												<span class="pl-pill alert">
													<span class="pl-alertdot" aria-hidden="true" />
													{formatCount(alerts())}{" "}
													{alerts() === 1 ? "alert" : "alerts"}
												</span>
											</Show>
											<Show when={spec.axes[1]}>
												<span class="pl-pill pl-axt">
													<AxisTitle parts={titleOf(spec.axes[1])} />
													<svg
														class="pl-axglyph"
														data-stroke="dotted"
														viewBox="0 0 12 8"
														aria-hidden="true"
													>
														<line x1="1" y1="4" x2="11" y2="4" />
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
								measured={readoutWidth() > 0}
								onWidth={setReadoutWidth}
								pinned={cursor()?.pinned ?? false}
								hint={!(cursor()?.pinned || byKeys())}
								single={lines().length === 1}
								names={names()}
								styles={styles()}
								hash={data()?.reports[current().report]?.hash?.slice(0, 7)}
								href={reportLink(current().report)}
								alert={pinnedAlert()}
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
					shared={named().shared}
					alerting={(line) => alerting(lines()[line])}
					max={keyMax()}
					compact={size() === "tile"}
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
						const clear = resolved() === line;
						if (props.hidden.includes(id)) {
							props.onHiddenChange(
								props.hidden.filter((hiddenId) => hiddenId !== id),
							);
						}
						props.onFocusChange(clear ? null : id);
					}}
					onPreview={setPreview}
				/>
			</Show>
		</figure>
	);
};

const reservedWidth = (
	data: PlotData,
	lines: readonly number[],
	units: string,
	mode: PlotScale,
	font: string,
) => {
	const drawn = axisTicks(
		lines.flatMap((line) => data.lines[line] ?? []),
		units,
		mode,
	);
	return drawn
		? axisWidth(formatTicks(drawn.ticks, drawn.step, drawn.tier), font)
		: axisWidth([], font);
};

/** Each measure's axis tier, from the lines it shows, or all its lines when none is shown. */
const axisTiers = (
	data: PlotData,
	extents: readonly ([number, number] | undefined)[],
	hidden: ReadonlySet<number>,
	mode: PlotScale,
): AxisTier[] =>
	data.measures.map((measure, index) => {
		let shown: [number, number] | undefined;
		let all: [number, number] | undefined;
		for (let at = 0; at < data.lines.length; at++) {
			const range = extents[at];
			if (data.lines[at]?.measure === index && range) {
				all = widen(all, range);
				if (!hidden.has(at)) {
					shown = widen(shown, range);
				}
			}
		}
		const range = shown ?? all;
		return range
			? rangeTier(range[0], range[1], measure.units, mode)
			: axisTier(0, measure.units);
	});

const widen = (
	range: [number, number] | undefined,
	[min, max]: [number, number],
): [number, number] =>
	range ? [Math.min(range[0], min), Math.max(range[1], max)] : [min, max];

const sameTiers = (a: readonly AxisTier[], b: readonly AxisTier[]) =>
	a.length === b.length &&
	a.every(
		(tier, index) =>
			tier.factor === b[index]?.factor &&
			tier.symbol === b[index]?.symbol &&
			tier.exponent === b[index]?.exponent,
	);

interface ReadoutProps {
	readout: NonNullable<ReturnType<typeof readout>>;
	placement: {
		x: number;
		top: number;
		ring: { top: number } | null;
	} | null;
	left: number;
	/** Its width is known, so `left` is where it sits. */
	measured: boolean;
	onWidth: (width: number) => void;
	/** Only a pinned readout takes the pointer, so only its link and close button work. */
	pinned: boolean;
	/** A click would pin the readout, so it says so. */
	hint: boolean;
	single: boolean;
	names: readonly LineName[];
	styles: ReturnType<typeof lineStyles>;
	hash: string | undefined;
	href: string | undefined;
	/** The pinned point's alert: its page, and Dismiss or Reactivate. */
	alert:
		| {
				name: string;
				href: string;
				action: "Dismiss" | "Reactivate" | undefined;
				refused: boolean;
				run: () => void;
		  }
		| undefined;
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
	let rows: HTMLDivElement | undefined;
	// The row the cursor reads stays in view when the rows scroll; a pin reads
	// out again, so it comes here too once the alert's line takes their room.
	createEffect(
		on(
			() => props.readout,
			() => {
				const row = rows?.querySelector<HTMLElement>(".pl-rrow.focused");
				if (!(rows && row) || rows.scrollHeight <= rows.clientHeight) {
					return;
				}
				const box = rows.getBoundingClientRect();
				const { top, bottom } = row.getBoundingClientRect();
				if (top < box.top) {
					rows.scrollTop -= box.top - top;
				} else if (bottom > box.bottom) {
					rows.scrollTop += bottom - box.bottom;
				}
			},
		),
	);
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
					// Inside the plots, never over the key below them.
					"max-height": `calc(100% - ${(props.placement?.top ?? 0) + 4}px)`,
					// Placed by its width, so it shows once measured rather than move.
					visibility: props.measured ? undefined : "hidden",
				}}
			>
				<span class="pl-when">
					<Show when={props.pinned && props.href} fallback={when()}>
						{(href) => <a href={href()}>{when()}</a>}
					</Show>
				</span>
				{/* Many lines or iterations scroll here, under the time and above the controls. */}
				<div class="pl-rows" ref={rows}>
					<For each={props.readout.rows}>
						{(row) => {
							const name = () => (
								<Show when={!props.single && props.styles[row.line]}>
									{(current) => (
										<span class="pl-rname ui-parts">
											<Swatch
												slot={current().slot}
												shape={current().shape}
												stroke={current().axis === 1 ? "dotted" : "solid"}
											/>
											<Show when={current().ordinal}>
												{(ordinal) => (
													<span
														class="pl-ord"
														data-fixed
														data-ordinal={ordinal()}
													/>
												)}
											</Show>
											<Show when={props.names[row.line]}>
												{(line) => <NameParts name={line()} />}
											</Show>
										</span>
									)}
								</Show>
							);
							const delta = () => (
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
							);
							const limit = () => (
								<Show when={row.limit}>
									<span class="pl-limit">{row.limit}</span>
								</Show>
							);
							return (
								<Show
									when={row.iterations}
									fallback={
										<div
											class="pl-rrow ui-parts"
											classList={{ focused: row.focused }}
										>
											{name()}
											<b class="pl-mono">{row.value}</b>
											{delta()}
											{limit()}
										</div>
									}
								>
									{(iterations) => (
										<>
											<Show when={!props.single}>
												<div class="pl-rrow ui-parts">{name()}</div>
											</Show>
											<For each={iterations()}>
												{(each) => (
													<div
														class="pl-rrow"
														classList={{ focused: each.emphasized }}
													>
														<span class="pl-it">
															iteration {each.iteration}
														</span>
														<b class="pl-mono">{each.value}</b>
														<Show when={each.alerted}>
															<span
																class="pl-alertdot"
																role="img"
																aria-label="alerted"
															/>
														</Show>
														<Show when={each.emphasized}>{delta()}</Show>
													</div>
												)}
											</For>
											<Show when={row.limit}>
												<div class="pl-rrow">{limit()}</div>
											</Show>
										</>
									)}
								</Show>
							);
						}}
					</For>
					<Show when={props.readout.more > 0}>
						<span class="pl-more">
							{props.readout.more} more{" "}
							{props.readout.more === 1 ? "line" : "lines"}
						</span>
					</Show>
				</div>
				<Show when={props.alert}>
					{(alert) => (
						<div class="pl-ralert">
							<a
								class="lnk"
								href={alert().href}
								aria-label={`Alert: ${alert().name}`}
							>
								<span class="pl-alertdot" aria-hidden="true" />
								Alert
							</a>
							<Show when={alert().action}>
								{(action) => (
									<>
										<span class="dotsep" aria-hidden="true">
											·
										</span>
										<button
											type="button"
											class="lnk"
											aria-label={`${action()} the alert on ${alert().name}`}
											onClick={() => alert().run()}
										>
											{action()}
										</button>
									</>
								)}
							</Show>
							<Show when={alert().refused}>
								<span class="pl-rerr" role="alert">
									The alert did not change
								</span>
							</Show>
						</div>
					)}
				</Show>
				<Show when={props.hint}>
					<span class="pl-pin">Click to pin</span>
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
