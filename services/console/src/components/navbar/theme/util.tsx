import { createMemo, createSignal } from "solid-js";
import {
	Theme,
	getColorScheme,
	getTheme,
	themeBackground,
	themeColor,
} from "./theme";

const [theme, setTheme] = createSignal(getColorScheme() ?? Theme.Light);
setInterval(() => {
	const newTheme = getTheme();
	if (newTheme && theme() !== newTheme) {
		setTheme(newTheme);
	}
}, 100);
export const themeSignal = theme;
export const getThemeColor = createMemo(() => themeColor(theme()));
export const getThemeBackground = createMemo(() => themeBackground(theme()));
