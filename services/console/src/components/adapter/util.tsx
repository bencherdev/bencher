import { createSignal } from "solid-js";
import type { Adapter } from "../../types/bencher";
import { getAdapter } from "./adapter";

const [adapter_inner, setAdapter] = createSignal<Adapter | null>(getAdapter());
setInterval(() => {
	const new_adapter = getAdapter();
	if (new_adapter !== adapter_inner()) {
		setAdapter(new_adapter);
	}
}, 100);

export const adapter = adapter_inner;
