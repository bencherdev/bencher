import { createEffect, createMemo, createSignal } from "solid-js";
import { useSearchParams } from "../../util/url";
import { ADAPTER_PARAM, getAdapter, storeAdapter, validAdapter } from "./adapter";
import type { Adapter } from "../../types/bencher";

export const currentAdapter = () => {
	const [searchParams, setSearchParams] = useSearchParams();

	const [init, setInit] = createSignal(false);
	createEffect(() => {
		if (init()) {
			return;
		}
		const initParams: Record<
			string,
			undefined | null | string
		> = {};
		if (!validAdapter(searchParams[ADAPTER_PARAM])) {
			initParams[ADAPTER_PARAM] = null;
		}

		if (Object.keys(initParams).length === 0) {
			setInit(true);
		} else {
			setSearchParams(initParams, { replace: true });
		}
	});

	const adapter = createMemo(() => searchParams[ADAPTER_PARAM]);

	const a = adapter();
	if (validAdapter(a)) {
		storeAdapter(a as Adapter);
		return a as Adapter;
	}
	if (typeof localStorage !== "undefined") {
		const a = getAdapter();
		if (validAdapter(a)) {
			return a as Adapter;
		}
	}
	return null;
};

const [adapter_inner, setAdapter] = createSignal<Adapter | null>(currentAdapter());
setInterval(() => {
	const new_adapter = getAdapter();
	if (new_adapter !== adapter_inner()) {
		setAdapter(new_adapter as Adapter | null);
	}
}, 100);

export const adapter = adapter_inner;