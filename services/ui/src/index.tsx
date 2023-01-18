/* @refresh reload */
import { render } from "solid-js/web";
import { Router } from "solid-app-router";
import { MDXProvider } from "solid-jsx";
import bencher_valid_init from "bencher_valid";

import App from "./App";
import mdx from "./mdx";
import { BENCHER_VERSION } from "./components/site/util";

// TODO get rid of the following warning:
// computations created outside a `createRoot` or `render` will never be disposed
// It seems like things are only being created once per full reload though,
// and this is the prescribed methodology from the `vite-plugin-wasm-pack` example:
// https://github.com/nshen/vite-plugin-wasm-pack/blob/main/example/src/index.ts
// https://stackoverflow.com/questions/70373659/solidjs-computations-created-outside-a-createroot-or-render-will-never-be
bencher_valid_init().then(() => {
	console.log(`🐰 Bencher v${BENCHER_VERSION}`);
	render(
		() => (
			<Router>
				<MDXProvider components={{ ...mdx }}>
					<App />
				</MDXProvider>
			</Router>
		),
		document.getElementById("root") as HTMLElement,
	);
});
