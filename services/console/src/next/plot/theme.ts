import { THEME_ATTRIBUTE } from "@bencherdev/ui/ThemeToggle";
import { type Accessor, createSignal, onCleanup } from "solid-js";
import { isServer } from "solid-js/web";
import { SLOTS } from "./series";

/** The tokens a canvas draws with, read once per theme. */
export interface Palette {
	series: readonly string[];
	dim: string;
	grid: string;
	plot: string;
	marker: string;
	onMarker: string;
	muted: string;
	faint: string;
	text: string;
	code: string;
}

const readPalette = (): Palette => {
	const style = getComputedStyle(document.documentElement);
	const token = (name: string) => style.getPropertyValue(name).trim();
	return {
		series: Array.from({ length: SLOTS }, (_, index) =>
			token(`--color-data-categorical-${index + 1}`),
		),
		dim: token("--color-data-dim"),
		grid: token("--color-data-grid"),
		plot: token("--color-background-plot"),
		marker: token("--color-marker"),
		onMarker: token("--color-on-marker"),
		muted: token("--color-text-muted"),
		faint: token("--color-text-faint"),
		text: token("--color-text-primary"),
		code: token("--font-family-code"),
	};
};

// Nothing draws on the server, where there are no tokens to read.
const SERVER: Palette = {
	series: [],
	dim: "",
	grid: "",
	plot: "",
	marker: "",
	onMarker: "",
	muted: "",
	faint: "",
	text: "",
	code: "",
};

const [palette, setPalette] = createSignal<Palette>(SERVER);
let observer: MutationObserver | undefined;
let users = 0;

/** The current palette, shared by every plot and redrawn when the theme changes. */
export const usePalette = (): Accessor<Palette> => {
	if (isServer) {
		return palette;
	}
	users++;
	if (!observer) {
		setPalette(readPalette());
		observer = new MutationObserver(() => setPalette(readPalette()));
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: [THEME_ATTRIBUTE],
		});
	}
	onCleanup(() => {
		users--;
		if (users === 0) {
			observer?.disconnect();
			observer = undefined;
		}
	});
	return palette;
};
