import type { JSX } from "solid-js";
import { splitProps } from "solid-js";
import Button from "./Button";
import Icon from "./Icon";

/** The root attribute the theme keys on; light is opt in and dark is the default. */
export const THEME_ATTRIBUTE = "data-theme";

export interface ThemeToggleProps
	extends JSX.ButtonHTMLAttributes<HTMLButtonElement> {
	/** The localStorage key that remembers the reader's choice. */
	storageKey: string;
}

/** Switches the page between light and dark, and remembers the choice. */
const ThemeToggle = (props: ThemeToggleProps) => {
	const [local, rest] = splitProps(props, ["storageKey"]);
	const toggle = () => {
		const root = document.documentElement;
		const next =
			root.getAttribute(THEME_ATTRIBUTE) === "light" ? "dark" : "light";
		root.setAttribute(THEME_ATTRIBUTE, next);
		try {
			localStorage.setItem(local.storageKey, next);
		} catch {
			// Blocked storage keeps the choice for this page only.
		}
	};
	return (
		<Button
			variant="secondary"
			{...rest}
			aria-label="Switch between light and dark"
			onClick={toggle}
		>
			<Icon name="theme" />
		</Button>
	);
};

export default ThemeToggle;
