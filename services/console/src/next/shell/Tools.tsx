import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import ThemeToggle from "@bencherdev/ui/ThemeToggle";
import { THEME_KEY } from "../theme";

/** One element: Docs and the theme toggle inline when wide, a popover on a phone. */
const Tools = () => (
	<>
		<Button
			variant="ghost"
			iconOnly
			class="tools-menu"
			popovertarget="tools"
			aria-label="Menu"
		>
			<Icon name="menu" />
		</Button>
		<div id="tools" class="tools ui-menu" popover="auto">
			<a class="docs" href="/docs/">
				Docs
			</a>
			<ThemeToggle storageKey={THEME_KEY} />
		</div>
	</>
);

export default Tools;
