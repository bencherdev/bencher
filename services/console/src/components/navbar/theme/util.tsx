import { createMemo, createSignal } from "solid-js";
import { Theme, getTheme, themeBackground, themeColor } from "./theme";

const [theme, setTheme] = createSignal(getTheme() ?? Theme.Light);
setInterval(() => {
	const newTheme = getTheme();
	if (newTheme && theme() !== newTheme) {
		setTheme(newTheme);
	}
}, 100);
export const themeSignal = theme;
export const getThemeColor = createMemo(() => themeColor(theme()));
export const getThemeBackground = createMemo(() => themeBackground(theme()));
