import { BENCHER_WORDMARK, BENCHER_WORDMARK_DARK } from "../../../util/ext";

export const THEME_KEY = "theme";

export enum Theme {
	Light = "light",
	Dark = "dark",
}

export const THEME_TOGGLE_ID = "theme-toggle";

export enum ThemeId {
	Light = "light-theme",
	Dark = "dark-theme",
}

export const getTheme = () => {
	if (typeof localStorage !== "undefined") {
		const theme = localStorage.getItem(THEME_KEY);
		switch (theme) {
			case Theme.Light:
			case Theme.Dark:
				return theme;
			case null:
				break;
			default:
				localStorage.removeItem(THEME_KEY);
		}
	}
	if (window.matchMedia("(prefers-color-scheme: dark)").matches) {
		return Theme.Dark;
	}
	return Theme.Light;
};

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

export const themeWordmark = (theme: Theme) => {
	switch (theme) {
		case Theme.Light:
			return BENCHER_WORDMARK;
		case Theme.Dark:
			return BENCHER_WORDMARK_DARK;
	}
}
