import {
	createMemo,
	type Accessor,
	getOwner,
	runWithOwner,
	on,
	untrack,
} from "solid-js";

export type Params = Record<string, string>;
export declare type SetParams = Record<
	string,
	string | number | boolean | null | undefined
>;

export interface Path {
	pathname: string;
	search: string;
	hash: string;
}

export interface Location<S = unknown> extends Path {
	query: Params;
	state: Readonly<Partial<S>> | null;
	key: string;
}

export interface NavigateOptions<S = unknown> {
	resolve: boolean;
	replace: boolean;
	scroll: boolean;
	state: S;
}

export const windowLocation = createMemo(() => window.location.toString());
export const windowState = createMemo(() => window.history.state);
export const useLocation = <S = unknown>() =>
	createLocation(windowLocation, windowState) as Location<S>;

export const useNavigate = () => {
	return (to: string | number, _options?: Partial<NavigateOptions>) => {
		// https://developer.mozilla.org/en-US/docs/Web/API/History/pushState
		const to_str = to.toString();
		const state = { path: to_str };
		window.history.pushState(state, "", to_str);
	};
};

// export const useSearchParams = (): [ () => URLSearchParams, (params: SetParams) => void] => {
//     const searchParams = () => new URL(document.location.toString()).searchParams;
//     const setSearchParams = (params: SetParams) => {
//         const url = new URL(document.location.toString());
//         for (const [key, value] of Object.entries(params)) {
// 			if (value) {
// 				url.searchParams.set(key, value.toString());
// 			}
// 		}
//         // https://developer.mozilla.org/en-US/docs/Web/API/History/pushState
//         const state = {path: url.toString()};
//         window.history.pushState(state, '', url);
//     };
//     return [searchParams, setSearchParams];
// };

export const useSearchParams = <T extends Params>(): [
	T,
	(params: SetParams, options?: Partial<NavigateOptions>) => void,
] => {
	const location = useLocation();
	const navigate = useNavigate();
	const setSearchParams = (
		params: SetParams,
		options?: Partial<NavigateOptions>,
	) => {
		const searchString = untrack(() =>
			mergeSearchString(location.search, params),
		);
		navigate(location.pathname + searchString + location.hash, {
			scroll: false,
			resolve: false,
			...options,
		});
	};
	return [location.query as T, setSearchParams];
};

export function extractSearchParams(url: URL): Params {
	const params: Params = {};
	url.searchParams.forEach((value, key) => {
		params[key] = value;
	});
	return params;
}

export function mergeSearchString(search: string, params: SetParams) {
	const merged = new URLSearchParams(search);
	Object.entries(params).forEach(([key, value]) => {
		if (value == null || value === "") {
			merged.delete(key);
		} else {
			merged.set(key, String(value));
		}
	});
	const s = merged.toString();
	return s ? `?${s}` : "";
}

export function createLocation(
	path: Accessor<string>,
	state: Accessor<any>,
): Location {
	const origin = new URL("http://sar");
	const url = createMemo<URL>(
		(prev) => {
			const path_ = path();
			try {
				return new URL(path_, origin);
			} catch (err) {
				console.error(`Invalid path ${path_}`);
				return prev;
			}
		},
		origin,
		{
			equals: (a, b) => a.href === b.href,
		},
	);

	const pathname = createMemo(() => url().pathname);
	const search = createMemo(() => url().search, true);
	const hash = createMemo(() => url().hash);
	const key = createMemo(() => "");

	return {
		get pathname() {
			return pathname();
		},
		get search() {
			return search();
		},
		get hash() {
			return hash();
		},
		get state() {
			return state();
		},
		get key() {
			return key();
		},
		query: createMemoObject(
			on(search, () => extractSearchParams(url())) as () => Params,
		),
	};
}

export function createMemoObject<T extends Record<string | symbol, unknown>>(
	fn: () => T,
): T {
	const map = new Map();
	const owner = getOwner()!;
	return new Proxy(<T>{}, {
		get(_, property) {
			if (!map.has(property)) {
				runWithOwner(owner, () =>
					map.set(
						property,
						createMemo(() => fn()[property]),
					),
				);
			}
			return map.get(property)();
		},
		getOwnPropertyDescriptor() {
			return {
				enumerable: true,
				configurable: true,
			};
		},
		ownKeys() {
			return Reflect.ownKeys(fn());
		},
	});
}

export const hiddenRedirect = (url: string): void => {
	window.location.replace(url);
};
