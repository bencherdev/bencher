import { createRoot, createSignal, onCleanup } from "solid-js";
import type { Adapter } from "../../types/bencher";
import { getAdapter } from "./adapter";

export const adapter = createRoot(() => {
	const [adapter, setAdapter] = createSignal<Adapter | null>(getAdapter());
	const interval = setInterval(() => {
		createRoot(() => {
			const new_adapter = getAdapter();
			if (new_adapter !== adapter()) {
				setAdapter(new_adapter);
			}
		});
	}, 100);

	onCleanup(() => clearInterval(interval));

	return adapter;
});
