/** A view whose code loads once, on first intent; a failed load tries again on the next. */
export const lazyView = <View>(load: () => Promise<{ default: View }>) => {
	let loaded: View | undefined;
	let loading: Promise<View> | undefined;
	const start = () => {
		loading ??= load().then(
			({ default: view }) => {
				loaded = view;
				return view;
			},
			(error: unknown) => {
				loading = undefined;
				throw error;
			},
		);
		return loading;
	};
	return {
		loaded: () => loaded,
		load: start,
		preload: () => {
			start().catch(() => {});
		},
	};
};
