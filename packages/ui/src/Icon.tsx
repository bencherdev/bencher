import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

/**
 * The enumerated glyph set. Each name carries one meaning. A new name enters
 * the set the way a new component does, when a screen needs it.
 */
export type IconName =
	| "alerts"
	| "check"
	| "chevron-down"
	| "close"
	| "key"
	| "read-only"
	| "theme"
	| "user"
	| "warning";

// One stroke-drawn set on a shared 24 grid: fill none, round joins, a
// single stroke width, all on currentColor so the glyph takes the token
// color of its context. Each entry is a function so every render gets its
// own DOM nodes rather than sharing one.
const GLYPHS: Record<IconName, () => JSX.Element> = {
	alerts: () => (
		<>
			<path d="M18 8a6 6 0 0 0-12 0c0 7-3 9-3 9h18s-3-2-3-9" />
			<path d="M13.7 21a2 2 0 0 1-3.4 0" />
		</>
	),
	check: () => <path d="M5 12.5l4.5 4.5L19 7.5" />,
	"chevron-down": () => <path d="M6 9l6 6 6-6" />,
	close: () => <path d="M6 6l12 12M18 6L6 18" />,
	key: () => (
		<>
			<circle cx="8" cy="15" r="4" />
			<path d="M11 12l9-9M17 6l3 3M15 8l2 2" />
		</>
	),
	"read-only": () => (
		<>
			<rect x="5" y="11" width="14" height="10" rx="2" />
			<path d="M8 11V8a4 4 0 0 1 8 0v3" />
		</>
	),
	theme: () => (
		<>
			<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" />
		</>
	),
	user: () => (
		<>
			<circle cx="12" cy="8" r="4" />
			<path d="M4 21c0-4 3.6-7 8-7s8 3 8 7" />
		</>
	),
	warning: () => <path d="M12 3l9.5 17h-19zM12 10v4M12 17.5v.01" />,
};

export interface IconProps
	extends Omit<JSX.SvgSVGAttributes<SVGSVGElement>, "children"> {
	/** Which enumerated glyph to draw. */
	name: IconName;
	/** Size on the type scale; omit to inherit the surrounding font size. */
	size?: "sm" | "md" | "lg" | "xl";
}

/**
 * A stroke-drawn icon on `currentColor`, sized to the type scale. Icons
 * carry enumerated meaning, never decoration. Decorative by default
 * (`aria-hidden`): give the control that wraps it the accessible label.
 */
const Icon = (props: IconProps) => {
	const [local, rest] = splitProps(props, ["name", "size", "class"]);
	return (
		<svg
			class={local.class === undefined ? "ui-icon" : `ui-icon ${local.class}`}
			data-icon={local.name}
			data-size={local.size}
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
			aria-hidden="true"
			{...rest}
		>
			{GLYPHS[local.name]()}
		</svg>
	);
};

export default Icon;
