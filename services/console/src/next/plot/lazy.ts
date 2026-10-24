type PlotModule = typeof import("./Plot");
type PlotView = PlotModule["default"];

let view: PlotView | undefined;
let loading: Promise<PlotView> | undefined;
// The URL of the module that failed to load, for a later try to ask for again.
let failed: string | undefined;
let tries = 0;

const MODULE_URL = /https?:\/\/\S+?\.js\b/;

/** The plot's code, with its charting library: loaded once, on first need, and again on a later call after a failure. */
export const loadPlot = () => {
	loading ??= (failed === undefined ? import("./Plot") : retry(failed)).then(
		({ default: loaded }) => {
			view = loaded;
			return loaded;
		},
		(error: unknown) => {
			loading = undefined;
			failed ??=
				error instanceof Error
					? MODULE_URL.exec(error.message)?.[0]
					: undefined;
			throw error;
		},
	);
	return loading;
};

// A function of its own, so the minifier cannot fold this import and the plot's into one call, which would keep only this one's empty preload list.
const retry = (url: string) =>
	import(/* @vite-ignore */ `${url}?try=${++tries}`) as Promise<PlotModule>;

/** The plot's code if it has loaded. */
export const loadedPlot = () => view;
