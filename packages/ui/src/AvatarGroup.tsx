import type { JSX } from "solid-js";
import { splitProps } from "solid-js";

export type AvatarGroupProps = JSX.HTMLAttributes<HTMLSpanElement>;

/** Overlapping avatars: seen receipts, participants. */
const AvatarGroup = (props: AvatarGroupProps) => {
	const [local, rest] = splitProps(props, ["class"]);
	return (
		<span
			class={
				local.class === undefined
					? "ui-avatar-group"
					: `ui-avatar-group ${local.class}`
			}
			{...rest}
		/>
	);
};

export default AvatarGroup;
