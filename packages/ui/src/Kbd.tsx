import type { JSX } from "solid-js";

/** A keyboard key, drawn as the key itself: a shortcut hint beside the thing
 * the key summons. Decorative by default; pair it with real text when the
 * hint carries meaning a screen reader needs. */
const Kbd = (props: JSX.HTMLAttributes<HTMLElement>) => (
	<kbd class="ui-kbd" {...props} />
);

export default Kbd;
