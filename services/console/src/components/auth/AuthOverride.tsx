import { authUser } from "../../util/auth";
import type { DOMElement, JSX } from "solid-js/jsx-runtime";

export interface Props {
	elementId: string;
	children: JSX.Element;
}

const AuthOverride = (props: Props) => {
	if (authUser()?.token) {
		const element = document.getElementById(props.elementId);
		const children = props.children as DOMElement;
		if (element && children) {
			element.innerHTML = children.innerHTML;
		}
	}
	return <></>;
};

export default AuthOverride;
