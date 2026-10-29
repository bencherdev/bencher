import init, { new_slug } from "bencher_valid";
import wasm from "bencher_valid/bencher_valid_bg.wasm?url";
import { createSignal } from "solid-js";

/** The slug the API makes from a name, by the API's own rule; nothing when it makes a random one. */
export type SlugRule = (name: string) => string | undefined;

let loading: Promise<SlugRule> | undefined;

const load = (): Promise<SlugRule> =>
	(loading ??= init({ module_or_path: wasm }).then(
		() => (name: string) => new_slug(name),
	));

/** The API's slug rule once its code has loaded; until then, or if it fails, nothing. */
export const useSlugRule = () => {
	const [rule, setRule] = createSignal<SlugRule>();
	load().then(
		(loaded) => setRule(() => loaded),
		() => {},
	);
	return rule;
};
