import {
	type Accessor,
	createComputed,
	createSignal,
	on,
	onCleanup,
} from "solid-js";

/** How long a query waits for the reader to stop editing. */
const DEBOUNCE = 200;

/**
 * The query the plot asks for. One whose answer the cache holds is asked for
 * at once, so Back draws without a wait; any other waits until the reader
 * pauses, so a run of edits makes one request.
 */
export const useRequested = <Q>(
	query: Accessor<Q>,
	key: (query: Q) => string,
	cached: (query: Q) => boolean,
) => {
	const [requested, setRequested] = createSignal(query());
	let timer: ReturnType<typeof setTimeout> | undefined;
	onCleanup(() => clearTimeout(timer));
	createComputed(
		on(
			() => key(query()),
			() => {
				clearTimeout(timer);
				const next = query();
				if (cached(next)) {
					setRequested(() => next);
				} else {
					timer = setTimeout(() => setRequested(() => next), DEBOUNCE);
				}
			},
			{ defer: true },
		),
	);
	return {
		requested,
		/** The query shown has not been asked for yet. */
		pending: () => key(query()) !== key(requested()),
	};
};
