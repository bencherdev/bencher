import {
	type Accessor,
	createEffect,
	createMemo,
	createSignal,
	on,
	onCleanup,
	onMount,
} from "solid-js";
import { ROW_HEIGHT, type Range, visibleRange } from "./rows";

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
	const [lines, setLines] = createSignal(1);
	const [rowHeight, setRowHeight] = createSignal<number>(ROW_HEIGHT[1]);
	const [range, setRange] = createSignal<Range>({ start: 0, end: 0 });
	const [edge, setEdge] = createSignal(0);
	/** The height of the first `count` rows. */
	const spaceFor = (count: number) =>
		count > 0 ? count * rowHeight() - edge() : 0;
	let body: HTMLTableSectionElement | undefined;
	let onScreen: Range = { start: 0, end: 0 };
	/** The lines of the rows the row height was measured from. */
	let held: number | undefined;
	/** The first row on screen as last measured. */
	let mark: Mark | undefined;

	const measure = () => {
		if (!body) {
			return;
		}
		const rows = body.querySelectorAll<HTMLElement>("tr[aria-rowindex]");
		const [first, second] = rows;
		// The table's first row has no top border, and a row opened to its whole
		// name grows, so the step is the shorter of two taken past the first row.
		const step = (above?: HTMLElement, below?: HTMLElement) =>
			above && below
				? below.getBoundingClientRect().top - above.getBoundingClientRect().top
				: Number.POSITIVE_INFINITY;
		const steps = Math.min(
			step(rows[1], rows[2]),
			step(rows[rows.length - 2], rows[rows.length - 1]),
		);
		const drawn = Number.isFinite(steps)
			? steps
			: second
				? step(first, second)
				: first?.getBoundingClientRect().height;
		const height = drawn && drawn > 0 ? drawn : heightOf(lines());
		// WebKit snaps table rows to steps half a pixel apart, which a spacer
		// multiplies, so a measure holds within a pixel of the last.
		if (!(drawn && held === lines() && Math.abs(height - rowHeight()) < 1)) {
			setRowHeight(height);
			held = drawn ? lines() : undefined;
		}
		// The first row is short of the step by its missing top border.
		const short = steps - step(first, second);
		if (range().start === 0 && Number.isFinite(steps) && short >= 0) {
			setEdge(short);
		}
		// Drawn at the expected height until a row exists to measure.
		if (!drawn && props.rows().length > 0) {
			schedule();
		}
		const place = {
			top: body.getBoundingClientRect().top,
			viewport: window.innerHeight,
			rowHeight: rowHeight(),
			count: props.rows().length,
		};
		const current = range();
		const next = visibleRange({ ...place, overscan: OVERSCAN });
		if (next.start !== current.start || next.end !== current.end) {
			setRange(next);
		}
		const visible = visibleRange({ ...place, overscan: 0 });
		if (visible.start !== onScreen.start || visible.end !== onScreen.end) {
			onScreen = visible;
			props.onRange?.(visible);
		}
		mark = markRows(body);
	};

	let frame: number | undefined;
	const schedule = () => {
		frame ??= requestAnimationFrame(() => {
			frame = undefined;
			measure();
		});
	};
	const shown = createMemo(() =>
		props.rows().slice(range().start, range().end),
	);
	onMount(() => {
		window.addEventListener("scroll", schedule, { passive: true });
		window.addEventListener("resize", schedule, { passive: true });
		// A web font that lands moves the rows by less than the hold, so they
		// measure afresh, and the window follows the row at the screen's top.
		const fonts = () => {
			held = undefined;
			const last = mark;
			const step = rowHeight();
			const screen = -(body?.getBoundingClientRect().top ?? 0);
			measure();
			if (!(body && last)) {
				return;
			}
			// Rows scrolled past since the last measure sit a step apart.
			const at = Math.max(
				0,
				last.index + Math.floor((screen - last.top) / step),
			);
			const row = body.querySelector(`tr[aria-rowindex="${at + 2}"]`);
			const now = row
				? row.getBoundingClientRect().top - body.getBoundingClientRect().top
				: spaceFor(at);
			window.scrollBy({
				top: now - last.top - (at - last.index) * step,
				behavior: "instant",
			});
		};
		document.fonts?.addEventListener("loadingdone", fonts);
		onCleanup(() => {
			document.fonts?.removeEventListener("loadingdone", fonts);
			window.removeEventListener("scroll", schedule);
			window.removeEventListener("resize", schedule);
			if (frame !== undefined) {
				cancelAnimationFrame(frame);
			}
		});
	});
	createEffect(on([() => props.rows().length, lines], measure));

	return {
		/** Hand the table's `onLines` here. */
		onLines: setLines,
		/** Hand the table's body here. */
		body: (element: HTMLTableSectionElement) => {
			body = element;
		},
		/** Where the rows drawn start. */
		start: () => range().start,
		shown,
		range,
		rowHeight,
		spaceFor,
		above: () => spaceFor(range().start),
		below: () => (props.rows().length - range().end) * rowHeight(),
	};
};

const heightOf = (lines: number) =>
	ROW_HEIGHT[lines as keyof typeof ROW_HEIGHT] ?? ROW_HEIGHT[1];

interface Mark {
	row: Element;
	/** Its index in the list. */
	index: number;
	/** Its top in the table's body. */
	top: number;
}

/** The first of the body's rows on screen, where it sits in the body. */
export const markRows = (body: HTMLElement | undefined): Mark | undefined => {
	const top = body?.getBoundingClientRect().top ?? 0;
	for (const row of body?.querySelectorAll("tr[aria-rowindex]") ?? []) {
		const box = row.getBoundingClientRect();
		if (box.bottom > 0) {
			return {
				row,
				index: Number(row.getAttribute("aria-rowindex")) - 2,
				top: box.top - top,
			};
		}
	}
	return undefined;
};

/** Scroll the window by how far the marked row has moved in the body since. */
export const holdRows = (body: HTMLElement | undefined, mark?: Mark) => {
	if (body && mark?.row.isConnected) {
		window.scrollBy({
			top:
				mark.row.getBoundingClientRect().top -
				body.getBoundingClientRect().top -
				mark.top,
			behavior: "instant",
		});
	}
};
