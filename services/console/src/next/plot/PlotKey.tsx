import Tooltip from "@bencherdev/ui/Tooltip";
import Truncate from "@bencherdev/ui/Truncate";
import { createSignal, For, Index, Show } from "solid-js";
import type { LineName, LineNames } from "./key";
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
	shared: LineNames["shared"];
	/** Whether a line has an alert still active. */
	alerting: (line: number) => boolean;
	/** Entries shown before the rest fold behind Show all. */
	max: number;
	compact: boolean;
	onToggle: (line: number) => void;
	onFocus: (line: number) => void;
	/** A line focused while its entry is hovered or holds keyboard focus. */
	onPreview: (line: number | null) => void;
}

const plural = (count: number) => (count === 1 ? "line" : "lines");

/** A rest this short never shrinks, so every entry keeps what tells it apart. */
const FIXED_REST = 32;

/** A line's name as parts of the label around it: its rest, whole when short unless `cut`, and the tags and details that give way first. */
export const NameParts = (props: { name: LineName; cut?: boolean }) => (
	<>
		<Show when={props.name.rest}>
			{(rest) => (
				<Show
					when={props.cut === true || rest().length > FIXED_REST}
					fallback={
						<span class="pl-rest" data-fixed>
							{rest()}
						</span>
					}
				>
					<Truncate class="pl-rest" text={rest()} tail />
				</Show>
			)}
		</Show>
		<Index each={props.name.tags}>
			{(tag) => (
				<>
					{" "}
					<span class="pl-tag">{tag()}</span>
				</>
			)}
		</Index>
		<Index each={props.name.detail}>
			{(detail) => (
				<>
					{" "}
					<Truncate class="pl-km" text={detail()} tail />
				</>
			)}
		</Index>
	</>
);

const Dot = () => (
	<span class="pl-dot" data-fixed aria-hidden="true">
		·
	</span>
);

export const PlotKey = (props: PlotKeyProps) => {
	const [open, setOpen] = createSignal(false);
	const shown = () =>
		open() || props.order.length <= props.max
			? props.order
			: props.order.slice(0, props.max);
	return (
		<div class="pl-key" classList={{ compact: props.compact }}>
			<div class="pl-khead ui-parts">
				<span data-fixed>
					<b>{props.lines.length}</b> {plural(props.lines.length)}
				</span>
				<Show when={props.hidden.size > 0}>
					<Dot />
					<span data-fixed>{props.hidden.size} hidden</span>
				</Show>
				<Show when={props.shared.benchmark}>
					{(benchmark) => (
						<>
							<Dot />
							<Truncate class="pl-mono" text={benchmark()} tail reveal />
						</>
					)}
				</Show>
				<Show when={props.shared.parameters.length > 0}>
					<Dot />
					<Truncate
						class="pl-mono"
						text={props.shared.parameters.join(" ")}
						reveal
					/>
				</Show>
			</div>
			<div class="pl-kgrid">
				<For each={shown()}>
					{(line) => {
						const name = () => props.names[line] as LineName;
						const style = () => props.styles[line] as LineStyle;
						const hidden = () => props.hidden.has(line);
						const target = () =>
							props.focused === line ? "Clear focus" : "Focus line";
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
								<Tooltip tip={hidden() ? "Show line" : "Hide line"}>
									<button
										type="button"
										class="pl-ktog ui-parts"
										aria-label={`${name().text}: ${hidden() ? "show" : "hide"} its line`}
										onClick={() => props.onToggle(line)}
									>
										<Show when={!hidden()} fallback={<EyeOff />}>
											<Swatch
												slot={style().slot}
												shape={style().shape}
												stroke={style().axis === 1 ? "dotted" : "solid"}
											/>
										</Show>
										<Show when={style().ordinal}>
											{(ordinal) => (
												<span
													class="pl-ord"
													data-fixed
													data-ordinal={ordinal()}
												/>
											)}
										</Show>
										<NameParts name={name()} />
										<Show when={props.alerting(line)}>
											<span class="pl-alertdot" aria-hidden="true" />
										</Show>
									</button>
								</Tooltip>
								<Tooltip tip={target()}>
									<button
										type="button"
										class="pl-kfoc"
										aria-label={`${target()} of ${name().text}`}
										onClick={() => props.onFocus(line)}
									>
										<Target />
									</button>
								</Tooltip>
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
