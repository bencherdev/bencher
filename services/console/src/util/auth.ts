import { createRoot, createSignal, onCleanup } from "solid-js";
import { READER_KEY, readReader } from "../next/reader";

/** The signed in reader, as the hint names them; a sign in or out in another tab changes it. */
export const authUser = createRoot(() => {
	const [reader, setReader] = createSignal(readReader(localStorage));
	const changed = (event: StorageEvent) => {
		if (event.key === READER_KEY || event.key === null) {
			setReader(readReader(localStorage));
		}
	};
	window.addEventListener("storage", changed);
	onCleanup(() => window.removeEventListener("storage", changed));
	return reader;
});
