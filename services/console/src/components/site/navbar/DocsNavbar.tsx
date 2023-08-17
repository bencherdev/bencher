import ConsoleNavbar from "./ConsoleNavbar";
import type { Params } from "astro";
import { authUser } from "../../../util/auth";
import type { DOMElement } from "solid-js/jsx-runtime";

export interface Props {
	params: Params;
}

const DocsNavbar = (props: Props) => {
	if (authUser()?.token) {
		const navbar = document.getElementById("bencher_navbar");
		const consoleNavbar = (
			<ConsoleNavbar params={props.params} />
		) as DOMElement;
		if (navbar && consoleNavbar) {
			navbar.innerHTML = consoleNavbar.innerHTML;
		}
	}
};

export default DocsNavbar;
