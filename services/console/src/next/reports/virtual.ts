import {
	type Accessor,
	createEffect,
	createMemo,
	createSignal,
	on,
	onCleanup,
	onMount,
} from "solid-js";
import { NARROW, useNarrow } from "../plot/narrow";
import { type Range, batchSize, visibleRange } from "./rows";

/** Rows drawn above and below the screen, so a quick scroll never shows a gap. */
const OVERSCAN = 8;

/**
 * The rows of a long table to draw: those on screen and a few either side,
 * measured from the rows drawn, with spacers of their height holding the rest.
 * `body` is the table's body, whose rows carry `aria-rowindex`.
 */
export const useVirtualRows = <T>(props: {
	rows: Accessor<readonly T[]>;
	/** The rows on screen changed, not counting those drawn either side. */
	onRange?: ((range: Range) => void) | undefined;
}) => {
	const narrow = useNarrow();
	const [rowHeight, setRowHeight] = createSignal<number>(ROW_HEIGHT.wide);
	const [range, setRange] = createSignal<Range>({ start: 0, end: 0 });
	let body: HTMLTableSectionElement | undefined;
	let onScreen: Range = { start: 0, end: 0 };

	const measure = () => {
		if (!body) {
			return;
		}
		const rows = body.querySelectorAll<HTMLElement>("tr[aria-rowindex]");
		const [first, second] = rows;
		const drawn =
			first && second
				? second.getBoundingClientRect().top - first.getBoundingClientRect().top
				: first?.getBoundingClientRect().height;
		const height =
			drawn && drawn > 0
				? drawn
				: narrow()
					? ROW_HEIGHT.narrow
					: ROW_HEIGHT.wide;
		setRowHeight(height);
		// Drawn at the expected height until a row exists to measure.
		if (!drawn && props.rows().length > 0) {
			schedule();
		}
		const place = {
			top: body.getBoundingClientRect().top,
			viewport: window.innerHeight,
			rowHeight: height,
			count: props.rows().length,
		};
		const next = visibleRange({ ...place, overscan: OVERSCAN });
		const current = range();
		if (next.start !== current.start || next.end !== current.end) {
			setRange(next);
		}
		const visible = visibleRange({ ...place, overscan: 0 });
		if (visible.start !== onScreen.start || visible.end !== onScreen.end) {
			onScreen = visible;
			props.onRange?.(visible);
		}
	};

	let frame: number | undefined;
	const schedule = () => {
		frame ??= requestAnimationFrame(() => {
			frame = undefined;
			measure();
		});
	};
	onMount(() => {
		window.addEventListener("scroll", schedule, { passive: true });
		window.addEventListener("resize", schedule, { passive: true });
		onCleanup(() => {
			window.removeEventListener("scroll", schedule);
			window.removeEventListener("resize", schedule);
			if (frame !== undefined) {
				cancelAnimationFrame(frame);
			}
		});
	});
	createEffect(on([() => props.rows().length, narrow], measure));

	return {
		/** Hand the table's body here. */
		body: (element: HTMLTableSectionElement) => {
			body = element;
		},
		/** Where the rows drawn start. */
		start: () => range().start,
		shown: createMemo(() => props.rows().slice(range().start, range().end)),
		above: () => range().start * rowHeight(),
		below: () => (props.rows().length - range().end) * rowHeight(),
	};
};

/** The fixed height a row is drawn at, wide and narrow. */
const ROW_HEIGHT = { wide: 40, narrow: 60 } as const;

/** The size of every batch on this screen: what fills it, plus a margin. */
export const screenBatch = () =>
	batchSize(
		window.innerHeight,
		window.matchMedia(NARROW).matches ? ROW_HEIGHT.narrow : ROW_HEIGHT.wide,
	);
