import { For, type JSX } from "solid-js";
import { PROJECTS_PATH } from "../paths";
import { SECTIONS, type Section } from "./section";

/**
 * The Settings sections: a rail beside the page on a wide screen, a
 * segmented row above it on a narrow one.
 */
const Frame = (props: {
	slug: string;
	section: Section | undefined;
	children: JSX.Element;
}) => {
	const links = () => (
		<For each={SECTIONS}>
			{({ section, label, path }) => (
				<a
					href={`${PROJECTS_PATH}/${props.slug}/${path}`}
					aria-current={section === props.section ? "page" : undefined}
				>
					{label}
				</a>
			)}
		</For>
	);
	return (
		<main class="page setwrap">
			<nav class="rail" aria-label="Settings">
				{links()}
			</nav>
			<nav class="railseg" aria-label="Settings sections">
				{links()}
			</nav>
			<div class="setbody">{props.children}</div>
		</main>
	);
};

export default Frame;
