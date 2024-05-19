import { createMemo, createSignal } from "solid-js";
import { Theme, getTheme, themeColor } from "./theme";

const [theme, setTheme] = createSignal(Theme.Light);
setInterval(() => {
	const newTheme = getTheme();
	if (theme() != newTheme) {
		setTheme(newTheme);
	}
}, 100);
export const themeSignal = theme;
export const getThemeColor = createMemo(() => themeColor(theme()));