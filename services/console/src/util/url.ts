/// Nearly all of this code is ripped from solid-js-router
/// https://github.com/solidjs/solid-router
/// However, since the underlying router is not being used
/// A kludge to simply poll for the current window location and state is used
import {
	createMemo,
	type Accessor,
	getOwner,
	runWithOwner,
	on,
	untrack,
	createSignal,
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

const [windowLocation, setWindowLocation] = createSignal<string>(
	window.location.toString(),
);
setInterval(() => {
	const location_str = window.location.toString();
	if (location_str !== windowLocation()) {
		setWindowLocation(location_str);
	}
}, 100);

const [windowState, setWindowState] = createSignal<any>(window.history.state);
setInterval(() => {
	const state = window.history.state;
	if (state !== windowState()) {
		setWindowState(state);
	}
}, 100);

export const useLocation = <S = unknown>() =>
	createLocation(windowLocation, windowState) as Location<S>;

// No page refresh
export const useNavigateSoft = () => {
	return (to: string | number, _options?: Partial<NavigateOptions>) => {
		// https://developer.mozilla.org/en-US/docs/Web/API/History/pushState
		const to_str = to.toString();
		const state = { path: to_str };
		window.history.pushState(state, "", to_str);
	};
};

// Page refresh
export const useNavigate = () => {
	return (to: string | number, _options?: Partial<NavigateOptions>) => {
		window.location.assign(to.toString());
	};
};

export const useSearchParams = <T extends Params>(): [
	T,
	(params: SetParams, options?: Partial<NavigateOptions>) => void,
] => {
	const location = useLocation();
	const navigate = useNavigateSoft();
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

/// Bencher specific helpers

export const hiddenRedirect = (url: string): void => {
	window.location.replace(url);
};

export const pathname = createMemo(() => useLocation().pathname);

export const organizationSlug = createMemo(() => {
	const path = pathname()?.split("/");
	if (!path) {
		return null;
	}
	if (
		path.length < 5 ||
		path[0] ||
		path[1] !== "console" ||
		path[2] !== "organizations" ||
		!path[3] ||
		!path[4]
	) {
		return null;
	}
	return path[3];
});

export const projectSlug = createMemo(() => {
	const path = pathname()?.split("/");
	if (!path) {
		return null;
	}
	if (
		path.length < 5 ||
		path[0] ||
		path[1] !== "console" ||
		path[2] !== "projects" ||
		!path[3]
	) {
		return null;
	}
	return path[3];
});
