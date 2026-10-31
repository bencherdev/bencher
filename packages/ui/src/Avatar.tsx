import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

const graphemes = new Intl.Segmenter();

// The first `count` characters as a reader sees them, so an emoji or a flag stays whole.
const leading = (word: string, count: number): string =>
	Array.from(graphemes.segment(word), ({ segment }) => segment)
		.slice(0, count)
		.join("");

const initials = (name: string): string => {
	const words = name.split(/\s+/).filter((word) => word !== "");
	if (words.length === 0) {
		return "";
	}
	// A single word (e.g. "Bencher") reads as its first two letters; a multi-word
	// name (e.g. "Ada Lovelace") reads as one letter per word, up to two.
	if (words.length === 1) {
		return leading(words[0] ?? "", 2).toUpperCase();
	}
	return words
		.slice(0, 2)
		.map((word) => leading(word, 1).toUpperCase())
		.join("");
};

export interface AvatarProps extends JSX.HTMLAttributes<HTMLSpanElement> {
	/** The profile name; the avatar shows its initials. */
	name?: string;
	/** Literal glyphs instead of initials, e.g. an overflow count. */
	label?: string;
	size?: "sm" | "md";
}

/** A profile as initials. */
const Avatar = (props: AvatarProps) => {
	const [local, rest] = splitProps(props, ["name", "label", "size", "class"]);
	return (
		<span
			class={
				local.class === undefined ? "ui-avatar" : `ui-avatar ${local.class}`
			}
			data-size={local.size ?? "sm"}
			{...rest}
		>
			{local.label ?? initials(local.name ?? "")}
		</span>
	);
};

export default Avatar;
