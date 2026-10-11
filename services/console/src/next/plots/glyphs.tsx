import type { JSX } from "solid-js";

// The page's own glyphs, kept out of the design system's set, which every page loads.
const Glyph = (props: { children: JSX.Element }) => (
	<svg
		class="ui-icon"
		viewBox="0 0 24 24"
		width="16"
		height="16"
		fill="none"
		stroke="currentColor"
		stroke-width="2.2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		{props.children}
	</svg>
);

export const Plus = () => (
	<Glyph>
		<path d="M12 5v14M5 12h14" />
	</Glyph>
);

export const Up = () => (
	<Glyph>
		<path d="M12 19V5M6 11l6-6 6 6" />
	</Glyph>
);

export const Down = () => (
	<Glyph>
		<path d="M12 5v14M6 13l6 6 6-6" />
	</Glyph>
);

export const Reorder = () => (
	<Glyph>
		<path d="M8 4v16M4 8l4-4 4 4M16 20V4M12 16l4 4 4-4" />
	</Glyph>
);
