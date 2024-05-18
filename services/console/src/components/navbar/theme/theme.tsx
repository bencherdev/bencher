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
}