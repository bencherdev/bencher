import { createMemo, createRoot, createSignal, onCleanup } from "solid-js";
import {
	DATA_THEME,
	Theme,
	getTheme,
	themeBackground,
	themeColor,
} from "./theme";

export const theme = createRoot(() => {
	const [theme, setTheme] = createSignal(getTheme() ?? Theme.Light);
	const interval = setInterval(() => {
		const newTheme = getTheme();
		if (newTheme && theme() !== newTheme) {
			setTheme(newTheme);
			document.documentElement.setAttribute(DATA_THEME, newTheme);
		}
	}, 100);

	onCleanup(() => clearInterval(interval));

	return theme;
});
export const getThemeColor = createMemo(() => themeColor(theme()));
export const getThemeBackground = createMemo(() => themeBackground(theme()));
