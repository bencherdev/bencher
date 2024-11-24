import { createMemo, createSignal } from "solid-js";
import { Theme, getTheme, themeBackground, themeColor } from "./theme";

const [theme, setTheme] = createSignal(Theme.Light);
setInterval(() => {
	const newTheme = getTheme();
	if (theme() !== newTheme) {
		setTheme(newTheme);
		document.documentElement.setAttribute("data-theme", newTheme);
	}
}, 100);
export const themeSignal = theme;
export const getThemeColor = createMemo(() => themeColor(theme()));
export const getThemeBackground = createMemo(() => themeBackground(theme()));
