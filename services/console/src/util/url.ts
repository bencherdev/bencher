import { createStore } from "solid-js/store";

export declare type SetParams = Record<string, string | number | boolean | null | undefined>;

export const useSearchParams = (): [URLSearchParams, (params: SetParams) => void] => {
    const [searchParams] = createStore(new URL(document.location.toString()).searchParams);
    const setSearchParams = (params: SetParams) => {
        const url = new URL(document.location.toString());
        for (const [key, value] of Object.entries(params)) {
			if (value) {
				url.searchParams.set(key, value.toString());
			}
		}
        // https://developer.mozilla.org/en-US/docs/Web/API/History/pushState
        const state = {path: url.toString()};
        window.history.pushState(state, '', url);
    };
    return [searchParams, setSearchParams];
};