import { createSignal, For, Index, Show } from "solid-js";
import type { LineName } from "./key";
import type { LineStyle } from "./series";
import { EyeOff, Swatch, Target } from "./Swatch";
import type { PlotLine } from "./types";

export interface PlotKeyProps {
	lines: readonly PlotLine[];
	names: readonly LineName[];
	styles: readonly LineStyle[];
	/** Line indices in key order. */
	order: readonly number[];
	hidden: ReadonlySet<number>;
	/** The line whose focus target is pressed. */
	focused: number | null;
	constants: readonly string[];
	/** Entries shown before the rest fold behind Show all. */
	max: number;
	compact: boolean;
	hint: boolean;
	onToggle: (line: number) => void;
	onFocus: (line: number) => void;
	/** A line focused while its entry is hovered or holds keyboard focus. */
	onPreview: (line: number | null) => void;
}

const plural = (count: number) => (count === 1 ? "line" : "lines");

export const PlotKey = (props: PlotKeyProps) => {
	const [open, setOpen] = createSignal(false);
	const shown = () =>
		open() || props.order.length <= props.max
			? props.order
			: props.order.slice(0, props.max);
	return (
		<div class="pl-key" classList={{ compact: props.compact }}>
			<div class="pl-khead">
				<span>
					<b>{props.lines.length}</b> {plural(props.lines.length)}
				</span>
				<Show when={props.hidden.size > 0}>
					<span class="pl-dot" aria-hidden="true">
						·
					</span>
					<span>{props.hidden.size} hidden</span>
				</Show>
				<Show when={props.constants.length > 0}>
					<span class="pl-dot" aria-hidden="true">
						·
					</span>
					<span class="pl-ellipsis">
						<span class="pl-mono">{props.constants.join(" ")}</span> on every
						line, so entries leave it out
					</span>
				</Show>
				<Show when={props.hint}>
					<span class="pl-dot" aria-hidden="true">
						·
					</span>
					<span>a name hides its line, the target focuses it</span>
				</Show>
			</div>
			<div class="pl-kgrid">
				<For each={shown()}>
					{(line) => {
						const name = () => props.names[line] as LineName;
						const style = () => props.styles[line] as LineStyle;
						const hidden = () => props.hidden.has(line);
						return (
							<div
								class="pl-ke"
								classList={{
									foc: props.focused === line,
									off: hidden(),
								}}
								onPointerEnter={() => props.onPreview(line)}
								onPointerLeave={() => props.onPreview(null)}
								onFocusIn={(event) => {
									// A click focuses a button too, and should not leave its line focused.
									if (event.target.matches(":focus-visible")) {
										props.onPreview(line);
									}
								}}
								onFocusOut={() => props.onPreview(null)}
							>
								<button
									type="button"
									class="pl-ktog"
									aria-label={`${hidden() ? "Show" : "Hide"} ${name().text}`}
									onClick={() => props.onToggle(line)}
								>
									<Show when={!hidden()} fallback={<EyeOff />}>
										<Swatch
											slot={style().slot}
											shape={style().shape}
											stroke={style().axis === 1 ? "dotted" : "solid"}
										/>
									</Show>
									<b>{name().benchmark}</b>
									<Index each={name().tags}>
										{(tag) => <span class="pl-tag">{tag()}</span>}
									</Index>
									<Index each={name().detail}>
										{(detail) => <span class="pl-km">{detail()}</span>}
									</Index>
									<Show when={props.lines[line]?.alerting}>
										<span class="pl-alertdot" aria-hidden="true" />
									</Show>
								</button>
								<button
									type="button"
									class="pl-kfoc"
									aria-pressed={props.focused === line}
									aria-label={`Focus ${name().text} and draw its boundary`}
									onClick={() => props.onFocus(line)}
								>
									<Target />
								</button>
							</div>
						);
					}}
				</For>
			</div>
			<Show when={props.order.length > props.max}>
				<button
					type="button"
					class="pl-kmore"
					aria-expanded={open()}
					onClick={() => setOpen(!open())}
				>
					{open() ? "Show fewer" : `Show all ${props.order.length}`}
				</button>
			</Show>
		</div>
	);
};

/** The key's frame before data arrives, entry for entry, so the drawn key takes its place without a shift. */
export const PlotKeyReserve = (props: {
	lines: number;
	max: number;
	compact: boolean;
}) => (
	<div class="pl-key" classList={{ compact: props.compact }} aria-hidden="true">
		<div class="pl-khead">
			<span class="pl-skel-text" />
		</div>
		<div class="pl-kgrid">
			<For each={Array.from({ length: Math.min(props.lines, props.max) })}>
				{() => (
					<div class="pl-ke">
						<span class="pl-ktog">
							<span class="pl-skel-text" />
						</span>
						<span class="pl-kfoc" />
					</div>
				)}
			</For>
		</div>
		<Show when={props.lines > props.max}>
			<span class="pl-kmore" />
		</Show>
	</div>
);
