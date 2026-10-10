import { type Accessor, createSignal, onCleanup } from "solid-js";

/** Below this width a row's controls would run off screen, so the list folds its rows as on a phone. */
export const FOLD = "(max-width: 979px)";

/** Whether the list folds its rows. */
export const useFold = (): Accessor<boolean> => {
	const query = matchMedia(FOLD);
	const [fold, setFold] = createSignal(query.matches);
	const listener = (event: MediaQueryListEvent) => setFold(event.matches);
	query.addEventListener("change", listener);
	onCleanup(() => query.removeEventListener("change", listener));
	return fold;
};
