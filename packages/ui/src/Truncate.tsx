import type { JSX } from "solid-js";
import {
	children,
	createEffect,
	createMemo,
	createSignal,
	on,
	onCleanup,
	Show,
} from "solid-js";
import { watchSize } from "./resize";
import { tailOf } from "./tail";

export { tailOf };

export interface TruncateProps {
	text: string;
	/** Keep the segment after the last `::` or `/` whole, so only what comes before it shrinks. */
	tail?: boolean;
	/** A press opens the whole value in place; never inside `a`, `button`, or `[role^=menuitem]`. */
	reveal?: boolean;
	class?: string;
}

/** Measures a text's whole width, which its box's clip does not change. */
const range =
	typeof document === "undefined" ? undefined : document.createRange();
const wholeWidth = (text: Node) => {
	range?.selectNodeContents(text);
	return range?.getBoundingClientRect().width ?? 0;
};

/**
 * Long text on one line, ending in an ellipsis where its box cuts it. The
 * whole value is one text node, so copy, find, and screen readers read it as
 * written; a cut `tail` is drawn again, whole, after the ellipsis.
 */
const Truncate = (props: TruncateProps) => {
	// While cut, the text's whole width, which the root keeps as its own
	// intrinsic width (truncate.css); drawing the tail then never changes the
	// space it gets.
	const [cut, setCut] = createSignal<number>();
	const [open, setOpen] = createSignal(false);
	const [focused, setFocused] = createSignal(false);
	const [root, setRoot] = createSignal<HTMLElement>();
	const tail = createMemo(() => {
		const [head, rest] = tailOf(props.text);
		return props.tail === true && head !== "" && rest !== "" ? rest : undefined;
	});
	// Created once, so a swap between span and button moves it.
	const text = (<span>{props.text}</span>) as HTMLSpanElement;

	// Cut exactly when the whole text is wider than the root's box. Either
	// change leaves the root's intrinsic width as it was, so it is written at
	// once; only a new width for a text already cut, after a font loads, waits.
	const read = () => {
		const box = root()?.getBoundingClientRect().width;
		if (open() || box === undefined) {
			return undefined;
		}
		const whole = wholeWidth(text);
		const was = cut();
		if (whole <= box + 0.01) {
			return was === undefined ? undefined : () => setCut(undefined);
		}
		if (was === undefined) {
			return () => setCut(whole);
		}
		return Math.abs(was - whole) > 0.01
			? () => requestAnimationFrame(() => setCut(whole))
			: undefined;
	};
	createEffect(
		on(
			() => props.text,
			() => setCut(undefined),
			{ defer: true },
		),
	);
	createEffect(() => {
		const element = root();
		if (
			element !== undefined &&
			(tail() !== undefined || props.reveal === true)
		) {
			// A new text, or a reveal just closed, is measured afresh by the
			// first notification.
			void props.text;
			void open();
			onCleanup(watchSize(element, read));
		}
	});

	const classes = () =>
		props.class === undefined ? "ui-trunc" : `ui-trunc ${props.class}`;
	const marked = () => (cut() === undefined ? undefined : "");
	const drawn = () => (cut() === undefined ? undefined : tail());
	const held = () => {
		const width = cut();
		return width === undefined
			? undefined
			: { "--ui-trunc-whole": `${width}px` };
	};
	return (
		<Show
			when={
				props.reveal === true && (open() || cut() !== undefined || focused())
			}
			fallback={
				<span
					ref={setRoot}
					class={classes()}
					data-cut={marked()}
					data-tail={drawn()}
					style={held()}
				>
					{text}
				</span>
			}
		>
			<button
				ref={setRoot}
				type="button"
				class={classes()}
				aria-expanded={open()}
				data-open={open() ? "" : undefined}
				data-cut={marked()}
				data-tail={drawn()}
				style={held()}
				onClick={() => setOpen(!open())}
				onFocus={() => setFocused(true)}
				onBlur={() => setFocused(false)}
			>
				{text}
			</button>
		</Show>
	);
};

export default Truncate;

/** A string child as `Truncate tail`; any other child as it is. */
export const TruncateChild = (props: {
	children?: JSX.Element;
	reveal?: boolean;
}) => {
	const child = children(() => props.children);
	return (
		<Show when={typeof child() === "string"} fallback={child()}>
			<Truncate text={child() as string} tail reveal={props.reveal === true} />
		</Show>
	);
};
