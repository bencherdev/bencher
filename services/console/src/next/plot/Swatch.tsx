import type { Shape } from "./series";

const r1 = (value: number) => Math.round(value * 10) / 10;

const markPath = (x: number, y: number, shape: Shape, radius: number) => {
	switch (shape) {
		case "square":
			return `M${r1(x - radius)} ${r1(y - radius)}h${r1(2 * radius)}v${r1(2 * radius)}h${r1(-2 * radius)}Z`;
		case "triangle":
			return `M${r1(x)} ${r1(y - radius - 1)}L${r1(x + radius + 0.8)} ${r1(y + radius)}L${r1(x - radius - 0.8)} ${r1(y + radius)}Z`;
		default:
			return `M${r1(x - radius)} ${r1(y)}a${radius} ${radius} 0 1 0 ${r1(2 * radius)} 0a${radius} ${radius} 0 1 0 ${r1(-2 * radius)} 0Z`;
	}
};

/** A line's color, stroke, and point shape, as the plot draws them. */
export const Swatch = (props: {
	slot: number;
	shape: Shape;
	stroke?: "solid" | "dotted" | "dashed";
}) => (
	<svg class={`pl-sw pl-s${props.slot}`} viewBox="0 0 24 12" aria-hidden="true">
		<line
			x1="1"
			y1="6"
			x2="23"
			y2="6"
			class="pl-sw-line"
			data-stroke={props.stroke ?? "solid"}
		/>
		<path
			class="pl-sw-mark"
			data-shape={props.shape}
			d={markPath(12, 6, props.shape, 3)}
		/>
	</svg>
);

export const EyeOff = () => (
	<svg
		class="pl-eye"
		viewBox="-5 0 34 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		aria-hidden="true"
	>
		<path d="M3 3l18 18" />
		<path d="M10.6 5.1A10.8 10.8 0 0 1 12 5c6.4 0 10 7 10 7a17.6 17.6 0 0 1-3.2 4.1M6.6 6.6C3.8 8.3 2 12 2 12s3.6 7 10 7c1.8 0 3.4-.5 4.8-1.3" />
		<path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
	</svg>
);

export const Target = () => (
	<svg
		width="14"
		height="14"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="9" />
		<circle cx="12" cy="12" r="3.2" fill="currentColor" />
	</svg>
);
