type PlotView = typeof import("./Plot").default;

let view: PlotView | undefined;
let loading: Promise<PlotView> | undefined;

/** The plot's code, with its charting library: loaded once, on first need. */
export const loadPlot = () => {
	loading ??= import("./Plot").then(
		({ default: loaded }) => {
			view = loaded;
			return loaded;
		},
		(error: unknown) => {
			loading = undefined;
			throw error;
		},
	);
	return loading;
};

/** The plot's code if it has loaded. */
export const loadedPlot = () => view;
