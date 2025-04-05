import {
	BENCHER_WORDMARK,
	BENCHER_WORDMARK_DARK,
	BENCHER_WORDMARK_LIGHT,
} from "../../../util/ext";

// https://bulma.io/documentation/features/dark-mode/
export const DATA_THEME = "data-theme";

export const BENCHER_THEME_KEY = "BENCHER_THEME";
export const LIGHT_THEME = "light";
export const DARK_THEME = "dark";

export enum Theme {
	// biome-ignore lint/style/useLiteralEnumMembers: const reuse
	Light = LIGHT_THEME,
	// biome-ignore lint/style/useLiteralEnumMembers: const reuse
	Dark = DARK_THEME,
}

export const THEME_TOGGLE_ID = "theme-toggle";
export const LIGHT_THEME_ID = "light-theme";
export const DARK_THEME_ID = "dark-theme";

export enum ThemeId {
	// biome-ignore lint/style/useLiteralEnumMembers: const reuse
	Light = LIGHT_THEME_ID,
	// biome-ignore lint/style/useLiteralEnumMembers: const reuse
	Dark = DARK_THEME_ID,
}

export const getColorScheme = () => {
	if (window.matchMedia("(prefers-color-scheme: light)").matches) {
		return Theme.Light;
	}
	if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
		return Theme.Dark;
	}
	return;
};

export const getTheme = () => getCachedTheme() ?? getColorScheme();

const getCachedTheme = () => {
	if (typeof localStorage !== "undefined") {
		const theme = localStorage.getItem(BENCHER_THEME_KEY);
		switch (theme) {
			case Theme.Light:
			case Theme.Dark:
				return theme;
			case null:
				return null;
			default:
				removeTheme();
		}
	}
	return null;
};

export const storeTheme = (theme: Theme) =>
	window.localStorage.setItem(BENCHER_THEME_KEY, theme);

export const removeTheme = () =>
	window.localStorage.removeItem(BENCHER_THEME_KEY);

export const themeText = (theme: Theme) => {
	switch (theme) {
		case Theme.Light:
			return "has-text-dark";
		case Theme.Dark:
			return "has-text-light";
	}
};

export const themeColor = (theme: Theme) => {
	switch (theme) {
		case Theme.Light:
			return "is-light";
		case Theme.Dark:
			return "is-dark";
	}
};

export const themeWordmark = (theme: undefined | Theme) => {
	switch (theme) {
		case Theme.Light:
			return BENCHER_WORDMARK_LIGHT;
		case Theme.Dark:
			return BENCHER_WORDMARK_DARK;
		default:
			return BENCHER_WORDMARK;
	}
};

export const themeBackground = (theme: Theme) => {
	switch (theme) {
		case Theme.Light:
			return "has-background-white";
		case Theme.Dark:
			return "has-background-black";
	}
};
