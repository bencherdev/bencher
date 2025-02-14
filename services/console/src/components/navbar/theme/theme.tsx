import {
	BENCHER_WORDMARK,
	BENCHER_WORDMARK_DARK,
	BENCHER_WORDMARK_LIGHT,
} from "../../../util/ext";

export const BENCHER_THEME_KEY = "BENCHER_THEME";

export enum Theme {
	Light = "light",
	Dark = "dark",
}

export const THEME_TOGGLE_ID = "theme-toggle";

export enum ThemeId {
	Light = "light-theme",
	Dark = "dark-theme",
}

export const getColorScheme = () => {
	if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
		return Theme.Dark;
	}
	return Theme.Light;
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
				localStorage.removeItem(BENCHER_THEME_KEY);
		}
	}
	return null;
};

export const storeTheme = (theme: Theme) =>
	window.localStorage.setItem(BENCHER_THEME_KEY, theme);

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
