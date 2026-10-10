import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

/**
 * The enumerated glyph set. Each name carries one meaning. A new name enters
 * the set the way a new component does, when a screen needs it.
 */
export type IconName = "theme";

// One stroke-drawn set on a shared 24 grid: fill none, round joins, a
// single stroke width, all on currentColor so the glyph takes the token
// color of its context. Each entry is a function so every render gets its
// own DOM nodes rather than sharing one.
const GLYPHS: Record<IconName, () => JSX.Element> = {
	theme: () => (
		<>
			<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" />
		</>
	),
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
