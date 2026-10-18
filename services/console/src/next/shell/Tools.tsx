import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import ThemeToggle from "@bencherdev/ui/ThemeToggle";
import type { JSX } from "solid-js";
import { THEME_KEY } from "../theme";

/**
 * Docs and the theme toggle inline when wide, a popover on a phone. What it
 * holds sits after them when wide and before the menu's button on a phone.
 */
const Tools = (props: { children?: JSX.Element }) => (
	<>
		<div id="tools" class="tools ui-menu" popover="auto">
			<a class="docs" href="/docs/">
				Docs
			</a>
			<ThemeToggle storageKey={THEME_KEY} />
		</div>
		{props.children}
		<Button
			variant="ghost"
			iconOnly
			class="tools-menu"
			popovertarget="tools"
			aria-label="Menu"
		>
			<Icon name="menu" />
		</Button>
	</>
);

export default Tools;
