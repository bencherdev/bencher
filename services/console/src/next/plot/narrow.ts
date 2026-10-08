import { type Accessor, createSignal, onCleanup } from "solid-js";
import { isServer } from "solid-js/web";

export const NARROW = "(max-width: 767px)";

const [narrow, setNarrow] = createSignal(false);
let query: MediaQueryList | undefined;
let users = 0;
const listener = (event: MediaQueryListEvent) => setNarrow(event.matches);

/** Whether the screen is phone width, one media query shared by every plot and row. */
export const useNarrow = (): Accessor<boolean> => {
	if (isServer) {
		return narrow;
	}
	if (users === 0) {
		query ??= matchMedia(NARROW);
		setNarrow(query.matches);
		query.addEventListener("change", listener);
	}
	users++;
	onCleanup(() => {
		users--;
		if (users === 0) {
			query?.removeEventListener("change", listener);
		}
	});
	return narrow;
};
