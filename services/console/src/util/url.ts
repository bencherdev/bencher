import { createStore } from "solid-js/store";

export declare type SetParams = Record<string, string | number | boolean | null | undefined>;

export const useSearchParams = () => {
    const [searchParams] = createStore(new URL(document.location.toString()).searchParams);
    const setSearchParams = (params: SetParams) => {
        for (const [key, value] of Object.entries(params)) {
			if (value) {
				searchParams.set(key, value.toString());
			}
		}
    };
    return [searchParams, setSearchParams];
};