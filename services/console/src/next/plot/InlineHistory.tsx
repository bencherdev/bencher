import { createEffect, onMount } from "solid-js";
import { drawInline, type InlineLine, inlineGeometry } from "./inline";
import { useNarrow } from "./narrow";
import { usePalette } from "./theme";
import "./plot.css";

interface InlineHistoryProps {
	/** The shared x of the row's page, in milliseconds. */
	x: readonly number[];
	line: InlineLine;
	/** A dismissed row's history, drawn in the muted color. */
	muted?: boolean;
	label: string;
}

// The canvas's CSS box in plot.css matches these, so the frame holds before any script runs.
const SIZE = { width: 150, height: 30 };
const NARROW_SIZE = { width: 112, height: 24 };

/** A row's history over the page window, one small canvas per row so hundreds scroll smoothly. */
const InlineHistory = (props: InlineHistoryProps) => {
	const narrow = useNarrow();
	const palette = usePalette();
	let canvas: HTMLCanvasElement | undefined;
	const size = () => (narrow() ? NARROW_SIZE : SIZE);

	onMount(() => {
		createEffect(() => {
			const context = canvas?.getContext("2d");
			if (!canvas || !context) {
				return;
			}
			const { width, height } = size();
			const ratio = window.devicePixelRatio || 1;
			canvas.width = Math.round(width * ratio);
			canvas.height = Math.round(height * ratio);
			context.setTransform(ratio, 0, 0, ratio, 0, 0);
			drawInline(
				context,
				inlineGeometry(props.x, props.line, width, height),
				palette(),
				props.muted ?? false,
			);
		});
	});

	return (
		<canvas
			ref={canvas}
			class="pl-inline"
			role="img"
			aria-label={props.label}
		/>
	);
};

export default InlineHistory;
